use pulldown_cmark::{
    BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
    TextMergeStream,
};
use serde_json::{Value, json};

use crate::error::JrError;

/// Maximum ADF nesting depth. Forward (`markdown_to_adf` post-passes) and
/// reverse (`adf_to_text`) tree walkers both reject inputs exceeding this
/// depth to prevent stack overflow (SEC-001, CWE-674).
/// Value 256 provides a large margin over any legitimate human-authored
/// nesting while staying well below the stack-overflow threshold on the
/// configured 8 MB stack.
pub(crate) const MAX_ADF_DEPTH: usize = 256;

pub fn text_to_adf(text: &str) -> Value {
    // BC-7.2.011 EC-12 (S-522): no raw \r or \n may appear in any text node
    // (Jira rejects them). Apply the same normalization that Algorithm B uses
    // for block HTML, but only when the input actually contains control chars.
    //
    // Fast path: single-line input with no \r or \n.  Return the original
    // one-paragraph / one-text-node shape byte-identical (regression guard).
    if !text.contains('\r') && !text.contains('\n') {
        return json!({
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
        });
    }

    // Multi-line path (BC-7.2.011 EC-12 steps 2–5):
    //
    // Step 2: Strip trailing \r/\n (any count). Non-newline trailing
    // whitespace (spaces/tabs) is preserved verbatim.
    let stripped = text.trim_end_matches(['\n', '\r']);

    // Step 3: Two-pass normalization — \r\n→\n first, then lone \r→\n.
    // Do NOT use a ['\r','\n'] char-set split; that double-counts CRLF.
    let normalized = stripped.replace("\r\n", "\n").replace('\r', "\n");

    // Blank-line split: split on \n\n (consecutive blank lines collapse to
    // one paragraph boundary — split_terminator on the empty segment that
    // results from \n\n\n would give an empty middle block, so we collect
    // and filter empty blocks after splitting).
    let blocks: Vec<&str> = normalized.split("\n\n").collect();

    // Build paragraph nodes; skip any block that is entirely empty (produced
    // by consecutive blank lines or an all-newlines input).
    let paragraphs: Vec<Value> = blocks
        .into_iter()
        .filter_map(|block| {
            // Within a block, split on \n to get inline segments separated
            // by hardBreak nodes (Algorithm B step 4 logic).
            let segments: Vec<&str> = block.split('\n').collect();
            let len = segments.len();

            // Check whether ALL segments are empty (i.e. block is blank).
            if segments.iter().all(|s| s.is_empty()) {
                return None;
            }

            // Build inline content: alternate text + hardBreak nodes.
            let mut content: Vec<Value> = Vec::with_capacity(len * 2);
            for (i, seg) in segments.iter().enumerate() {
                if !seg.is_empty() {
                    content.push(json!({ "type": "text", "text": seg }));
                }
                if i < len - 1 {
                    content.push(json!({ "type": "hardBreak" }));
                }
            }

            // Trim leading/trailing hardBreak nodes (reuse existing helper).
            let content = trim_leading_trailing_hardbreaks(content);

            if content.is_empty() {
                None
            } else {
                Some(json!({
                    "type": "paragraph",
                    "content": content,
                }))
            }
        })
        .collect();

    // If all content was blank/stripped, produce a single paragraph with an
    // empty text node (mirrors the fast-path shape for text_to_adf("")).
    if paragraphs.is_empty() {
        return json!({
            "version": 1,
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        { "type": "text", "text": "" }
                    ]
                }
            ]
        });
    }

    json!({
        "version": 1,
        "type": "doc",
        "content": paragraphs,
    })
}

pub fn markdown_to_adf(markdown: &str) -> Result<Value, JrError> {
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
        | Options::ENABLE_GFM
        // GFM task lists (`- [ ] …` / `- [x] …`) → ADF `taskList`/`taskItem`
        // nodes with `state: "TODO"/"DONE"` and mandatory `localId` attrs.
        // pulldown emits `Event::TaskListMarker(bool)` after Start(Tag::Item):
        // in a TIGHT list it fires directly (item body is inline-only);
        // in a LOOSE list it fires inside Start(Paragraph) (item body is
        // paragraph-wrapped). The builder uses Approach B (post-hoc
        // reclassification): the retroactive stack mutation converts the
        // current Paragraph or ListItem to TaskItem when TaskListMarker fires;
        // at End(Tag::List) the BulletList inspects children for taskItem
        // candidates and reclassifies the whole container to `taskList`.
        // `taskItem.content` is inline-only (NO paragraph wrapper) — EC-16
        // inline-flattening strips paragraph wrappers for loose items.
        // Normalization: `listItem > taskList` is unwrapped; `blockquote >
        // taskList` is unwrapped to `blockquote > [paragraph, …]`. `panel >
        // taskList` passes through (panel.content permits taskList).
        // localIds assigned post-normalization via DFS pre-order walk (#471).
        // See docs/specs/adf-task-list.md.
        | Options::ENABLE_TASKLISTS;
    let parser = TextMergeStream::new(Parser::new_ext(markdown, options));
    let mut builder = AdfBuilder::new();
    for event in parser {
        builder.process(event);
    }
    let mut content = builder.finish()?;
    // Post-normalization DFS pre-order walk: assign monotonically increasing
    // 1-based counter strings ("1", "2", …) to all taskList.attrs.localId and
    // taskItem.attrs.localId fields. The walk runs AFTER finish() so that pruned
    // nodes (whose counter slots are reclaimed) do not participate. Container
    // nodes are numbered before their children (pre-order). No uuid crate (#471).
    assign_local_ids(&mut content)?;
    // pulldown-cmark 0.13 has no autolink extension (ENABLE_GFM only adds alert
    // blockquotes), so bare URLs arrive as plain text. Post-process the built
    // tree to apply `link` marks to explicit-scheme `http(s)://` runs — Jira's
    // REST API does NOT auto-linkify plain text, so the mark is required for the
    // URL to be clickable (#473, .factory/research/issue-473-bare-url-autolink-scope.md).
    autolink_bare_urls(&mut content, 0)?;
    Ok(json!({
        "version": 1,
        "type": "doc",
        "content": content,
    }))
}

/// Post-process an ADF node array, applying `link` marks to bare `http(s)://`
/// URLs found in plain `text` nodes. Scope is deliberately narrow (#473):
///
/// - **Explicit scheme only** (`http://` / `https://`). `www.`-prefixed hosts
///   and bare emails are out of scope — they require scheme inference and carry
///   a much higher false-positive rate in prose (version strings, file paths,
///   sentence-final domains). Since an applied mark permanently writes a link
///   into the user's issue, the narrowest scope that covers the common case wins.
/// - Text nodes already carrying a `link` mark (from `<url>` autolinks or
///   `[text](url)`) or a `code` mark (inline code) are left untouched.
/// - `codeBlock` content is never linkified (preformatted/code text).
///
/// A *subset* of the GFM autolink boundary + extent rules is applied (see the
/// "Deviations from GFM" section of `docs/specs/adf-bare-url-autolink.md`):
/// a URL may start only at the beginning of a text node or after whitespace /
/// `*_~(` (GFM's "before" set also admits `[` and `]`, which we deliberately omit
/// to cut false positives); trailing punctuation (`?!.,:*_~`) is excluded; a trailing
/// `)` is trimmed only when unbalanced. One inherent limitation of running over
/// the *already-built* tree: a URL whose interior contains inline markup (e.g.
/// `https://x/a*b*c`, where `*b*` parsed as emphasis) has already been split into
/// separate text nodes, so only the leading plain run is linked.
fn autolink_bare_urls(nodes: &mut Vec<Value>, depth: usize) -> Result<(), JrError> {
    if depth >= MAX_ADF_DEPTH {
        return Err(JrError::UserError(
            "markdown nesting too deep (max 256 levels)".to_string(),
        ));
    }
    let mut i = 0;
    while i < nodes.len() {
        let node_type = nodes[i].get("type").and_then(Value::as_str).unwrap_or("");
        match node_type {
            // Never linkify code — both block content and inline-code text nodes.
            "codeBlock" => {}
            "text" => {
                let has_link_or_code =
                    nodes[i]
                        .get("marks")
                        .and_then(Value::as_array)
                        .is_some_and(|ms| {
                            ms.iter().any(|m| {
                                matches!(
                                    m.get("type").and_then(Value::as_str),
                                    Some("link") | Some("code")
                                )
                            })
                        });
                if !has_link_or_code {
                    if let Some(replacement) = split_text_node_on_urls(&nodes[i]) {
                        let len = replacement.len();
                        nodes.splice(i..=i, replacement);
                        i += len;
                        continue;
                    }
                }
            }
            _ => {
                if let Some(content) = nodes[i].get_mut("content").and_then(Value::as_array_mut) {
                    autolink_bare_urls(content, depth + 1)?;
                }
            }
        }
        i += 1;
    }
    Ok(())
}

/// Split a plain `text` node into a run of text nodes where each bare-URL span
/// gains a `link` mark (preserving the node's existing inline marks). Returns
/// `None` when the text contains no bare URL, so the caller leaves the node as-is.
fn split_text_node_on_urls(node: &Value) -> Option<Vec<Value>> {
    let text = node.get("text").and_then(Value::as_str)?;
    let spans = find_bare_url_spans(text);
    if spans.is_empty() {
        return None;
    }
    let base_marks = node.get("marks").and_then(Value::as_array);
    let make_node = |slice: &str, is_link: bool| {
        let mut out = json!({ "type": "text", "text": slice });
        let mut marks: Vec<Value> = base_marks.cloned().unwrap_or_default();
        if is_link {
            marks.push(json!({ "type": "link", "attrs": { "href": slice } }));
        }
        if !marks.is_empty() {
            out["marks"] = json!(marks);
        }
        out
    };
    let mut result = Vec::new();
    let mut cursor = 0;
    for (start, end) in spans {
        if start > cursor {
            result.push(make_node(&text[cursor..start], false));
        }
        result.push(make_node(&text[start..end], true));
        cursor = end;
    }
    if cursor < text.len() {
        result.push(make_node(&text[cursor..], false));
    }
    Some(result)
}

/// Locate bare `http(s)://` URL byte-spans within `text`, applying GFM autolink
/// boundary and extent rules (explicit-scheme subset, #473).
fn find_bare_url_spans(text: &str) -> Vec<(usize, usize)> {
    // Scheme detection is case-insensitive (RFC 3986 / GFM treat URL schemes
    // case-insensitively, so `HTTPS://`, `Http://`, `httpS://` are all valid).
    // Search a lowercased copy: `to_ascii_lowercase` is a 1:1 byte-length-
    // preserving map (only ASCII A–Z fold; non-ASCII bytes are untouched), so
    // every offset into `lower` is a valid offset into `text`. Spans and hrefs
    // are sliced from the ORIGINAL `text`, preserving the user's path case.
    let lower = text.to_ascii_lowercase();
    let mut spans = Vec::new();
    let mut search = 0;
    while let Some(rel) = lower[search..].find("http") {
        let start = search + rel;
        let scheme_len = if lower[start..].starts_with("https://") {
            8
        } else if lower[start..].starts_with("http://") {
            7
        } else {
            search = start + 4;
            continue;
        };
        // GFM boundary: an autolink starts only at text-node start, or after
        // whitespace or one of `*`, `_`, `~`, `(`. (ASCII whitespace/punctuation
        // are identical in `text` and `lower`, so checking either is equivalent.)
        let boundary_ok = start == 0
            || text[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_whitespace() || matches!(c, '*' | '_' | '~' | '('));
        if !boundary_ok {
            search = start + scheme_len;
            continue;
        }
        // Extent: consume non-whitespace, non-`<` characters after the scheme.
        let mut end = start + scheme_len;
        for ch in text[start + scheme_len..].chars() {
            if ch.is_whitespace() || ch == '<' {
                break;
            }
            end += ch.len_utf8();
        }
        let trimmed = start + trim_url_extent(&text[start..end]);
        // Require at least one character past `scheme://` after trimming.
        if trimmed > start + scheme_len {
            spans.push((start, trimmed));
            search = trimmed;
        } else {
            search = start + scheme_len;
        }
    }
    spans
}

/// Given a candidate URL slice, return the byte length to keep after applying
/// GFM trailing-punctuation trimming and parenthesis balancing. Trailing
/// `?!.,:*_~` are excluded; a trailing `)` is trimmed only when the slice has
/// more `)` than `(` (so a balanced `…Foo_(bar)` keeps its parens). Iterates
/// until stable to handle combinations like `…example.com).`.
fn trim_url_extent(url: &str) -> usize {
    let mut end = url.len();
    loop {
        let trimmed = url[..end].trim_end_matches(['?', '!', '.', ',', ':', '*', '_', '~']);
        let mut new_end = trimmed.len();
        if trimmed.ends_with(')') {
            let opens = trimmed.matches('(').count();
            let closes = trimmed.matches(')').count();
            if closes > opens {
                new_end -= 1; // ')' is one byte
            }
        }
        if new_end == end {
            return end;
        }
        end = new_end;
    }
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
    // SEC-001: captures the first depth-limit error from the normalizer calls
    // inside `end()`. Propagated by `finish()` → `markdown_to_adf`.
    depth_error: Option<JrError>,
}

struct PartialNode {
    kind: NodeKind,
    children: Vec<Value>,
}

/// Typed return value for the `end()` match arm. Replaces the old
/// `_pending_hoists`/`_post_hoists` JSON side-channel that embedded
/// coordination state directly in the ADF value tree (a risk: any code path
/// that skipped stripping would leak a temp field → Jira 400 via
/// `additionalProperties: false`).
///
/// - `Single(node)` — emit one node, no siblings.
/// - `WithHoists { node, hoists }` — emit `node` FIRST, then each hoist in
///   order as siblings at the same parent level.
/// - `Empty` — emit nothing.
enum EndResult {
    Single(Value),
    WithHoists { node: Value, hoists: Vec<Value> },
    Empty,
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
    // ADF `taskItem` node. `checked` carries the `TaskListMarker(bool)` state:
    // `true` → `"DONE"`, `false` → `"TODO"` (uppercase). `is_task` marks that
    // this item received a `TaskListMarker` event; items without a marker are
    // promoted to `taskItem { state: "TODO" }` in mixed lists (EC-3).
    TaskItem { checked: bool },
    // Block-level HTML (`<div>x</div>` on its own line). ADF has no raw-HTML
    // node, so the verbatim source lines are preserved as literal text inside a
    // paragraph — symmetric with inline HTML — rather than silently dropped
    // (issue #489). The inner `Event::Html` lines accumulate as text children;
    // on End they are concatenated, the single trailing block newline trimmed,
    // and emitted as one `paragraph`.
    HtmlBlock,
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

/// Typed segment in a mixed task/plain-item list during BulletList reclassification.
///
/// Used by the `NodeKind::BulletList` arm of `AdfBuilder::end()` to preserve
/// source document order when task items and hoisted blocks are interleaved.
/// - `Task`: a `taskItem` or nested `taskList` — accumulated into a contiguous
///   `taskList` run.
/// - `Hoist`: any other block (e.g. `bulletList`, `orderedList`) — emitted as
///   a sibling, flushing any pending task run first.
#[derive(Debug)]
enum Segment {
    /// task-compatible node — goes into a taskList run
    Task(serde_json::Value),
    /// non-task block sibling — emitted as-is, flushing any pending task run
    Hoist(serde_json::Value),
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
            depth_error: None,
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
            // GFM task-list marker: arrives as the FIRST child event INSIDE
            // `Start(Tag::Item)` — i.e. AFTER `Start(Item)` has already been
            // processed. The ordering is:
            //   Start(Item) → TaskListMarker(bool) → Text(…) → End(Item)
            // So at this point the stack top is a `ListItem` node (just pushed
            // by `start(Tag::Item)`). Retroactively convert it to a `TaskItem`
            // by mutating the kind on the stack.
            // `true` → DONE, `false` → TODO. (#471, BC-7.2.010)
            Event::TaskListMarker(checked) => {
                if let Some(top) = self.stack.last_mut() {
                    top.kind = NodeKind::TaskItem { checked };
                }
            }
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
            // Block HTML: preserve the verbatim source as literal text rather
            // than discarding it (issue #489). The wrapped `Event::Html` lines
            // flow into this node via `push_text` and are finalized on End.
            Tag::HtmlBlock => self.push(NodeKind::HtmlBlock),
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
        // `result` carries the node(s) to emit. Using a typed `EndResult` instead
        // of embedding coordination state in the JSON avoids the risk of
        // `additionalProperties: false` Jira-400 from a leaked temp field.
        let result: EndResult = match kind {
            NodeKind::Paragraph => {
                EndResult::Single(json!({ "type": "paragraph", "content": children }))
            }
            NodeKind::Heading(level) => EndResult::Single(json!({
                "type": "heading",
                "attrs": { "level": level },
                "content": children,
            })),
            NodeKind::BlockQuote => {
                // ADF `blockquote.content` forbids `taskList`. pulldown-cmark 0.13.3
                // DOES emit `blockquote > taskList` for `> - [ ] item` — the task-marker
                // scan in firstpass.rs is container-agnostic. Normalize: unwrap taskList
                // children → each taskItem's inline content becomes a paragraph inside the
                // blockquote. (BC-7.2.010 obligation #2 / EC-6, unconditional.)
                let normalized = match normalize_blockquote_content(children, 0) {
                    Ok(v) => v,
                    Err(e) => {
                        if self.depth_error.is_none() {
                            self.depth_error = Some(e);
                        }
                        Vec::new()
                    }
                };
                EndResult::Single(json!({ "type": "blockquote", "content": normalized }))
            }
            NodeKind::Panel { panel_type } => {
                // ADF `panel.content` forbids nested `panel`, `table`, and
                // `blockquote`; normalize_panel_content transforms each into the
                // permitted set BEFORE wrapping loose inline runs (mirrors the
                // listItem path #470). See docs/specs/adf-panel-content-model.md.
                let normalized = match normalize_panel_content(children, 0) {
                    Ok(v) => v,
                    Err(e) => {
                        if self.depth_error.is_none() {
                            self.depth_error = Some(e);
                        }
                        Vec::new()
                    }
                };
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
                            // REQUIRED (#471 AC-008): panel.content permits taskList as a
                            // direct child (canonical @atlaskit/adf-schema full.json).
                            // Without this entry, a surviving taskList is misclassified as
                            // inline → wrapped into paragraph > taskList (INVALID ADF,
                            // Jira 400). Source: .factory/research/issue-471-panel-tasklist-shape.md §D.
                            "taskList",
                        ],
                    )
                };
                EndResult::Single(json!({
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
                EndResult::Single(node)
            }
            NodeKind::BulletList => {
                // Approach B post-hoc reclassification (BC-7.2.010): delegate to
                // the shared `reclassify_as_task_list` helper when the list contains
                // at least one `taskItem` child. The helper is symmetric with the
                // OrderedList arm — see its doc-comment for shape examples and the
                // decision rationale.
                //
                // ORDER-PRESERVING reclassification (F-PASS4-C1 fix):
                //
                // taskList.content permits only taskItem and nested taskList nodes.
                // Any other block child (bulletList, orderedList, or any other block
                // that was a sibling of a taskItem via EndResult::WithHoists) must be
                // hoisted to the parent level. The helper preserves document order:
                // task runs are flushed into taskList nodes and hoist blocks are
                // emitted as siblings in source order.
                //
                // Shape examples (order invariant: preserve document order):
                //   `- [ ]\n  - plain\n- [x] after`
                //     → [bulletList(plain), taskList([after])]
                //   `- [x] before\n- [ ]\n  - plain\n- [x] after`
                //     → [taskList([before]), bulletList(plain), taskList([after])]
                //   `- [ ] outer\n  - plain inner`
                //     → [taskList([outer]), bulletList(inner)] (EC-15, unchanged)
                //
                // BC back-propagation note: BC-7.2.010 does not specify the
                // interleaved shape. The implemented invariant is: output preserves
                // source document order; valid ADF; does not drop content.
                let has_task_items = children
                    .iter()
                    .any(|c| c.get("type").and_then(Value::as_str) == Some("taskItem"));
                if has_task_items {
                    // `reclassify_as_task_list` returns `Some` when has_task_items is
                    // true; `expect` is the idiomatic way to document the invariant.
                    reclassify_as_task_list(children).expect(
                        "reclassify_as_task_list must return Some when taskItem children exist",
                    )
                } else {
                    // Plain bulletList (no task items). However, when an empty
                    // taskItem was pruned by is_empty_block_container, its hoisted
                    // block children (e.g. a nested bulletList or taskList) were
                    // appended directly to OUR children via append_child — these are
                    // NOT valid bulletList children (only listItem is). Use the shared
                    // split+hoist helper to dissolve the list when all children are
                    // stray blocks.
                    //
                    // BC-7.2.010 EC-13/EC-15 with empty outer body (F-PASS3-C1):
                    // `- [ ]\n  - plain inner`  → [bulletList{inner}] hoisted to doc
                    // `- [ ]\n  - [x] nested`   → [taskList{inner}] hoisted to doc
                    // The outer list itself is dropped (no valid listItem children).
                    split_stray_blocks_end_result("bulletList", children, &mut |block| {
                        self.append_child(block);
                    })
                }
            }
            NodeKind::OrderedList { start } => {
                // Reclassify ordered lists containing task markers to `taskList`
                // (shared path with BulletList via `reclassify_as_task_list`).
                //
                // Decision: ADF has no ordered task list; `orderedList.content`
                // permits only `listItem` and rejects `taskItem` (Jira HTTP 400).
                // GFM's `1. [ ] x` renders as a checkbox list — ordinal numbering
                // is cosmetic for checked items. Promoting to `taskList` preserves
                // the user's checkbox intent and is symmetric with the bullet rule.
                // Ordinal numbering is dropped (lossy). A plain ordered list with
                // no task markers is unchanged.
                let has_task_items = children
                    .iter()
                    .any(|c| c.get("type").and_then(Value::as_str) == Some("taskItem"));
                if has_task_items {
                    reclassify_as_task_list(children).expect(
                        "reclassify_as_task_list must return Some when taskItem children exist",
                    )
                } else {
                    // Plain orderedList (no task items). Mirror the BulletList
                    // stray-block split+hoist path (F-P11-001): when an empty
                    // ordered task item is pruned its hoisted block children
                    // (e.g. a nested taskList) are appended directly to OUR
                    // children — these are NOT valid orderedList children (only
                    // listItem is). Use the shared helper to dissolve + hoist.
                    //
                    // When only valid listItem children remain and `start != 1`,
                    // re-attach the `order` attr to the produced node.
                    let result =
                        split_stray_blocks_end_result("orderedList", children, &mut |block| {
                            self.append_child(block);
                        });
                    if start != 1 {
                        // Attach `order` attr to the orderedList node if present.
                        match result {
                            EndResult::Single(mut node) => {
                                node["attrs"] = json!({ "order": start });
                                EndResult::Single(node)
                            }
                            EndResult::WithHoists { mut node, hoists } => {
                                node["attrs"] = json!({ "order": start });
                                EndResult::WithHoists { node, hoists }
                            }
                            other => other,
                        }
                    } else {
                        result
                    }
                }
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
                //
                // Loose task-list items (EC-16 loose case): in a loose list
                // (`- [ ] line1\n\n  line2`), pulldown-cmark wraps the task marker
                // AND the first paragraph body in `Tag::Paragraph`. The retroactive
                // stack mutation converts that `Paragraph` to `TaskItem`, so the
                // first child of this `ListItem` is already a `taskItem` JSON node.
                // Remaining paragraphs are plain `paragraph` children that need
                // EC-16 inline-flattening. Detect this case and produce a merged
                // `taskItem` directly (bypassing the listItem wrapping path).
                let is_loose_task = children
                    .first()
                    .and_then(|c| c.get("type"))
                    .and_then(|t| t.as_str())
                    == Some("taskItem");
                if is_loose_task {
                    // Extract state from the first (task) child.
                    let state = children[0]["attrs"]["state"]
                        .as_str()
                        .unwrap_or("TODO")
                        .to_owned();
                    // Collect all inline content: unwrap the first taskItem's
                    // content, then unwrap subsequent paragraphs, separating with
                    // hardBreak nodes. Block children (taskList, bulletList, …)
                    // are hoisted to the parent (same EC-15 hoist path as below).
                    //
                    // Note on F-471-M3: the Paragraph-converted-to-TaskItem (the first
                    // child here) is produced by a tight-sub-item's End(Paragraph), and
                    // pulldown-cmark emits all block content (nested lists, blockquotes,
                    // etc.) AFTER End(Paragraph) — so the first taskItem child has no
                    // block siblings from the event stream. Block children appear as
                    // separate entries in `children` (type "taskList", "bulletList", …),
                    // not nested inside the taskItem node. The `_ => hoisted.push(child)`
                    // arm below catches them.
                    //
                    // CR-003: reuse flatten_task_item_to_inline instead of a hand-rolled
                    // copy. We normalise the input so every inline-bearing child looks
                    // like a paragraph (flatten_task_item_to_inline unwraps paragraphs
                    // and injects hardBreak separators). Block children go to `hoisted`.
                    let mut flat_for_flatten: Vec<Value> = Vec::new();
                    let mut hoisted: Vec<Value> = Vec::new();
                    for child in children {
                        let ty = child.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match ty {
                            "taskItem" => {
                                // Re-wrap the taskItem's inline content as a paragraph so
                                // flatten_task_item_to_inline can extract it uniformly.
                                let content = child
                                    .get("content")
                                    .and_then(|c| c.as_array())
                                    .cloned()
                                    .unwrap_or_default();
                                if !content.is_empty() {
                                    flat_for_flatten
                                        .push(json!({ "type": "paragraph", "content": content }));
                                }
                            }
                            "paragraph" => flat_for_flatten.push(child),
                            _ => hoisted.push(child),
                        }
                    }
                    let merged = trim_leading_trailing_hardbreaks(flatten_task_item_to_inline(
                        flat_for_flatten,
                    ));
                    let node = json!({
                        "type": "taskItem",
                        "attrs": { "localId": "", "state": state },
                        "content": merged,
                    });
                    // F-1 (F5-pass6): mirror the tight-path WithHoists pattern.
                    // Previously this called self.append_child(hoist) BEFORE returning
                    // Single(taskItem), which placed hoists in BulletList.children BEFORE
                    // the taskItem (inverted order: [bulletList(inner), taskItem(outer)]).
                    // Using WithHoists lets the end() dispatch append node FIRST, then
                    // hoists — preserving source order: [taskItem(outer), bulletList(inner)].
                    if hoisted.is_empty() {
                        EndResult::Single(node)
                    } else {
                        EndResult::WithHoists {
                            node,
                            hoists: hoisted,
                        }
                    }
                } else {
                    let normalized = match normalize_list_item_content(children, 0) {
                        Ok(v) => v,
                        Err(e) => {
                            if self.depth_error.is_none() {
                                self.depth_error = Some(e);
                            }
                            Vec::new()
                        }
                    };
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
                    EndResult::Single(json!({ "type": "listItem", "content": wrapped }))
                }
            }
            NodeKind::TaskItem { checked } => {
                // EC-16 inline-flattening: taskItem.content is inline-only.
                // pulldown-cmark wraps item bodies in `Tag::Paragraph`, producing
                // `paragraph` children. Strip paragraph wrappers; concatenate
                // inline content from multiple paragraphs with a `hardBreak`
                // separator between them. Then apply the hardBreak-trim rule:
                // remove any leading or trailing hardBreak nodes, and any
                // hardBreak adjacent to a pruned-empty paragraph (which contributes
                // zero inline nodes). This MUST run before the prune gate below.
                //
                // Block children from nested sublists or other block constructs in
                // a tight task item arrive here as direct children (pulldown-cmark
                // emits them inside the item before End(Item) when the item body is
                // NOT wrapped in a paragraph — i.e. tight lists).
                //
                // Only `text` and `hardBreak` are valid inline nodes in
                // taskItem.content. ANY other node type (including codeBlock,
                // blockquote, heading, table, rule, panel, bulletList, orderedList,
                // taskList) is a block sibling that must be hoisted out. Using
                // EndResult::WithHoists, hoists are appended to the parent (BulletList)
                // AFTER the taskItem — the BulletList reclassification arm then
                // classifies them correctly (taskList → task_children for EC-13;
                // everything else → hoisted set for EC-15 hoist-to-grandparent).
                //
                // This is correct for ALL block types, not just the original narrow
                // match on taskList|bulletList|orderedList (F-471-H1 fix).
                let mut inline_children: Vec<Value> = Vec::new();
                let mut block_siblings: Vec<Value> = Vec::new();
                for child in children {
                    let ty = child.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    // `text` and `hardBreak` are the only truly inline ADF node types
                    // valid in taskItem.content. Everything else is a block that must
                    // be hoisted to the parent container.
                    match ty {
                        "text" | "hardBreak" => inline_children.push(child),
                        _ => block_siblings.push(child),
                    }
                }
                // F-P2-C1 fix: inline_children are ALREADY inline (text/hardBreak),
                // NOT paragraph-wrapped. Do NOT route through flatten_task_item_to_inline
                // — that function is for the loose/paragraph-wrapped multi-paragraph case
                // only (handled in NodeKind::ListItem). Calling it here on bare text/
                // hardBreak nodes caused its non-paragraph else branch to inject spurious
                // hardBreak separators between EVERY text node (e.g. `- [x] **bold** and
                // _em_` → [text("bold"), hardBreak, text(" and "), hardBreak, text("em")]).
                // Bare inline nodes are used directly; only the trim pass is needed to
                // clean any explicit hardBreak nodes at the boundaries.
                let trimmed = trim_leading_trailing_hardbreaks(inline_children);
                let state = if checked { "DONE" } else { "TODO" };
                let node = json!({
                    "type": "taskItem",
                    "attrs": { "localId": "", "state": state },
                    "content": trimmed,
                });
                // Return via typed channel: the node (possibly empty) plus any block
                // siblings. The dispatch block below handles the prune-but-still-hoist
                // case (F-471-M1): even if the taskItem body is empty (and will be
                // pruned), its block siblings (nested sublists) must still reach the
                // parent BulletList so EC-13/EC-15 can process them.
                if block_siblings.is_empty() {
                    EndResult::Single(node)
                } else {
                    EndResult::WithHoists {
                        node,
                        hoists: block_siblings,
                    }
                }
            }
            NodeKind::Table => EndResult::Single(json!({ "type": "table", "content": children })),
            NodeKind::TableRow => {
                EndResult::Single(json!({ "type": "tableRow", "content": children }))
            }
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
                EndResult::Single(json!({ "type": cell_type, "content": wrapped }))
            }
            NodeKind::InlineMark => {
                self.pop_mark();
                // InlineMark has no ADF node of its own. Splice children (already
                // tagged with marks at `push_text` time, plus any nested text or
                // hardBreak nodes from inner mark spans) into the parent.
                for child in children {
                    self.append_child(child);
                }
                EndResult::Empty
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
                EndResult::Empty
            }
            NodeKind::HtmlBlock => {
                // ADF has no raw-HTML node. Preserve block HTML as literal text
                // in a `paragraph`, using `hardBreak` nodes to represent interior
                // newlines (issue #492 — Algorithm B). This differs from inline
                // HTML in three load-bearing ways: (1) block HTML is wrapped in
                // its own manufactured `paragraph`; inline HTML flows into the
                // enclosing paragraph directly. (2) Trailing `\r`/`\n` are trimmed
                // from the block; inline HTML is not trimmed. (3) Block HTML carries
                // no `active_marks` (the mark stack is always empty when a
                // `HtmlBlock` end fires); inline HTML inherits the current mark stack.

                // Step 1: Concatenate the per-line `Event::Html` strings accumulated
                // as child text nodes during the HtmlBlock span.
                let mut text = String::new();
                for child in &children {
                    if let Some(s) = child.get("text").and_then(Value::as_str) {
                        text.push_str(s);
                    }
                }

                // Step 2: Trim trailing `\r` and `\n` (any count). Trailing
                // non-newline whitespace (spaces/tabs) is NOT trimmed — preserved
                // verbatim in the last text node (BC-7.2.011 step 2).
                let trimmed = text.trim_end_matches(['\n', '\r']);

                // Step 3: Normalize then split. Normalize `\r\n`→`\n` first, then
                // lone `\r`→`\n`, then split on `\n`. Do NOT use a `['\r','\n']`
                // char-set split — that double-counts CRLF boundaries, producing
                // spurious hardBreak nodes (BC-7.2.011 step 3 mandatory constraint).
                let normalized = trimmed.replace("\r\n", "\n").replace('\r', "\n");
                let segments: Vec<&str> = normalized.split('\n').collect();
                let len = segments.len();

                // Step 4: Walk segments by index. For each i: emit a text node if
                // the segment is non-empty; emit exactly ONE hardBreak if i < len-1
                // (one per split boundary, regardless of whether adjacent segments
                // are empty — the LOSSLESS rule from BC-7.2.011 step 4).
                // Build the content Vec directly — do NOT route through push_text
                // (which applies active_marks; HtmlBlock has no active marks, but
                // routing through it would also break the direct-content-array build).
                let mut content: Vec<Value> = Vec::with_capacity(len * 2);
                for (i, seg) in segments.iter().enumerate() {
                    if !seg.is_empty() {
                        content.push(json!({ "type": "text", "text": seg }));
                    }
                    if i < len - 1 {
                        content.push(json!({ "type": "hardBreak" }));
                    }
                }

                // Step 5 + 5b: Wrap in paragraph, then trim any leading/trailing
                // hardBreak nodes (leading ones can arise from a block whose
                // trimmed string begins with `\n`; trim_leading_trailing_hardbreaks
                // is the existing helper reused from the taskItem path).
                let content = trim_leading_trailing_hardbreaks(content);

                // Step 6: Early-return if the content array is empty after trimming.
                // (`paragraph` is excluded from `is_empty_block_container`'s
                // REQUIRES_CONTENT set — the early-return here is the operative prune.)
                if content.is_empty() {
                    EndResult::Empty
                } else {
                    EndResult::Single(json!({
                        "type": "paragraph",
                        "content": content,
                    }))
                }
                // Step 7: autolink_bare_urls runs automatically in the post-finish
                // pass over the built tree — no change needed here.
            }
            NodeKind::Sink => EndResult::Empty,
        };
        // Dispatch: emit node(s) to the parent container.
        //
        // For `Single` / `WithHoists`: drop block containers left with empty
        // `content` (invalid ADF that Jira rejects with HTTP 400). Two ways this
        // arises:
        //   * pulldown-cmark hoists a footnote definition out of an enclosing
        //     block, leaving an empty shell (`> [^1]: x` -> empty blockquote);
        //   * a contentless heading from a bare `#` line.
        // End events fire inner-first, so if a future transform emptied a nested
        // container it would be pruned before its parent finalizes. (In practice
        // today the only reachable empties are a direct blockquote and a bare
        // heading; the list path keeps a valid empty placeholder paragraph and is
        // never pruned — see is_empty_block_container.)
        //
        // For `WithHoists` specifically: if the node itself is pruned (empty body
        // — e.g. `- [ ]` with an empty task body but a nested sub-list), the hoists
        // STILL propagate to the parent (F-471-M1 fix). This preserves nested sub-
        // lists even when the outer task item had no text.
        match result {
            EndResult::Empty => {}
            EndResult::Single(node) => {
                if !is_empty_block_container(&node) {
                    self.append_child(node);
                }
            }
            EndResult::WithHoists { node, hoists } => {
                // Append the primary node first (if non-empty), then hoists.
                // The order [node, hoist1, hoist2, …] is preserved regardless of
                // whether the node itself is pruned.
                if !is_empty_block_container(&node) {
                    self.append_child(node);
                }
                for hoist in hoists {
                    // CR-002: prune individual hoists just as the primary node
                    // is pruned. No-op today (hoists are bulletList/orderedList/
                    // taskList — none empty in current paths), but guards future
                    // paths that might generate empty hoist containers.
                    if !is_empty_block_container(&hoist) {
                        self.append_child(hoist);
                    }
                }
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
        // Check stack top once for both the Sink guard and the context classification.
        enum TextCtx {
            CodeBlock,
            HtmlBlock,
            Other,
        }
        let ctx = match self.stack.last() {
            Some(top) if matches!(top.kind, NodeKind::Sink) => return,
            Some(top) if matches!(top.kind, NodeKind::CodeBlock { .. }) => TextCtx::CodeBlock,
            Some(top) if matches!(top.kind, NodeKind::HtmlBlock) => TextCtx::HtmlBlock,
            _ => TextCtx::Other,
        };
        // BC-7.2.011 EC-11: no text node may contain a raw \r or (outside codeBlock)
        // a raw \n. Mirror Algorithm B step-3 normalization so inline paths share the
        // same invariant as HtmlBlock.
        //
        // Context determines the replacement for CR/LF:
        // - CodeBlock: \r\n → \n, lone \r → \n  (codeBlock text nodes allow \n for
        //   multi-line preformatted content; Jira accepts embedded \n here).
        // - HtmlBlock: CR is left UNCHANGED. The HtmlBlock End arm (Algorithm B)
        //   accumulates the raw text strings from children and does its own
        //   \r\n→\n / lone \r→\n normalization before splitting into hardBreak
        //   nodes. Pre-normalizing here would turn a lone \r into a space, which
        //   Algorithm B would then NOT treat as a line boundary — breaking the
        //   hardBreak contract. HtmlBlock owns its own CR/LF lifecycle.
        // - Other (paragraph, heading, inline text): \r\n → space, lone \r → space,
        //   lone \n → space (mirrors SoftBreak → " "; raw \n is forbidden in
        //   non-codeBlock text nodes per INV-1 — this is the defense-in-depth
        //   chokepoint that catches bare \n from inline HTML and other inline paths).
        let normalized;
        let needs_norm =
            text.contains('\r') || (matches!(ctx, TextCtx::Other) && text.contains('\n'));
        let text = if needs_norm {
            match ctx {
                TextCtx::CodeBlock => {
                    normalized = text.replace("\r\n", "\n").replace('\r', "\n");
                    normalized.as_str()
                }
                TextCtx::HtmlBlock => text, // Algorithm B owns CR/LF normalization
                TextCtx::Other => {
                    // Replace CRLF pairs first (→ single space), then lone \r/\n.
                    normalized = text.replace("\r\n", " ").replace(['\r', '\n'], " ");
                    normalized.as_str()
                }
            }
        } else {
            text
        };
        let mut node = json!({ "type": "text", "text": text });
        if !self.active_marks.is_empty() {
            node["marks"] = json!(dedup_marks_by_type(&self.active_marks));
        }
        self.append_child(node);
    }

    /// Emit an inline `code`-marked text node for `Event::Code`.
    ///
    /// **BC-7.2.015 (issue #571) — code-mark exclusivity invariant:**
    /// The ADF schema (`code_inline_node`) only permits `link` and `annotation`
    /// marks alongside `code`. All typographic marks (`strong`, `em`, `strike`,
    /// `subsup`, `underline`, `textColor`, `backgroundColor`, and any future
    /// mark types not currently emitted by pulldown-cmark) are stripped at
    /// emission time. The filter is an allowlist: any mark type outside
    /// `{link, annotation}` is excluded.
    ///
    /// The filter operates on a **clone** of `self.active_marks`; the original
    /// is never mutated. Surrounding non-code text nodes in the same inline span
    /// therefore retain their typographic marks unchanged (see EC-6).
    ///
    /// The trailing `dedup_marks_by_type` call is retained unchanged — it
    /// preserves the BC-7.2.007 same-type dedup invariant on this code path.
    ///
    /// **BC-7.2.011 (issue #522) — CR/LF normalization:**
    /// No `text` node may contain a raw `\r` or `\n`. `\r\n`, lone `\r`, and
    /// bare `\n` are all normalized to a single space before building the node.
    fn push_code(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(top) = self.stack.last() {
            if matches!(top.kind, NodeKind::Sink) {
                return;
            }
        }
        // BC-7.2.011 EC-11: no text node may contain a raw \r or \n (same invariant as
        // push_text Other context). Structural precondition: push_code is only ever
        // called for inline Event::Code, which CommonMark guarantees never nests inside
        // a codeBlock or HtmlBlock — hence no context branch is needed here.
        // \r\n → space, lone \r → space, lone \n → space (defense-in-depth chokepoint
        // for INV-1; catches bare \n arriving from any inline path).
        let normalized;
        let text = if text.contains('\r') || text.contains('\n') {
            // Replace CRLF pairs first (→ single space), then lone \r/\n.
            normalized = text.replace("\r\n", " ").replace(['\r', '\n'], " ");
            normalized.as_str()
        } else {
            text
        };
        // BC-7.2.015 (issue #571): allowlist filter — retain only `link` and
        // `annotation` marks from active_marks; strip all typographic marks.
        // Operates on a clone so self.active_marks is not mutated.
        let mut marks: Vec<Value> = self
            .active_marks
            .iter()
            .filter(|m| {
                matches!(
                    m.get("type").and_then(|t| t.as_str()),
                    Some("link") | Some("annotation")
                )
            })
            .cloned()
            .collect();
        marks.push(json!({ "type": "code" }));
        self.append_child(json!({
            "type": "text",
            "text": text,
            "marks": dedup_marks_by_type(&marks),
        }));
    }

    fn finish(mut self) -> Result<Vec<Value>, JrError> {
        // INVARIANT: depth_error is checked FIRST and self.root may hold partial
        // content built up to the point where the depth limit was hit.  This early
        // return is what prevents that partial ADF from being returned to the caller.
        // Do NOT move this check below `Ok(self.root)`.
        if let Some(e) = self.depth_error {
            return Err(e);
        }
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
        Ok(self.root)
    }
}

/// Shared stray-block split+hoist logic for both `BulletList` and `OrderedList`
/// non-task-bearing else-branches (F-P11-001 / F-PASS3-C1 fix).
///
/// When an empty task item is pruned by `is_empty_block_container`, its hoisted
/// block children (e.g. a nested `bulletList` or `taskList`) are appended
/// directly to the enclosing list's children. Those blocks are NOT valid list
/// children (`bulletList` and `orderedList` permit only `listItem`).
///
/// This function splits the children into `listItem` nodes (valid) and stray
/// non-`listItem` blocks (invalid), then returns the appropriate `EndResult`:
///
/// - All empty → `Empty`
/// - No stray blocks → `Single(list)` with all `listItem` children
/// - Only stray blocks (no `listItem`) → hoists every stray block to the
///   grandparent via `caller_append` and returns `Empty`  (the list dissolves)
/// - Mixed → `WithHoists { node: list(listItems), hoists: stray_blocks }`
///
/// `list_type` is either `"bulletList"` or `"orderedList"`. For `orderedList`
/// the `start` attribute is optionally set by the caller after the fact if this
/// returns `Single`.
///
/// The `caller_append` closure is called for each stray block when there are
/// NO `listItem` children, mirroring the `self.append_child(block)` call in the
/// BulletList inline path that this function replaces.
fn split_stray_blocks_end_result(
    list_type: &str,
    children: Vec<Value>,
    caller_append: &mut dyn FnMut(Value),
) -> EndResult {
    let mut list_items: Vec<Value> = Vec::new();
    let mut stray_blocks: Vec<Value> = Vec::new();
    for child in children {
        let ty = child.get("type").and_then(Value::as_str).unwrap_or("");
        if ty == "listItem" {
            list_items.push(child);
        } else {
            stray_blocks.push(child);
        }
    }
    if list_items.is_empty() && stray_blocks.is_empty() {
        EndResult::Empty
    } else if stray_blocks.is_empty() {
        EndResult::Single(json!({ "type": list_type, "content": list_items }))
    } else if list_items.is_empty() {
        // No valid list items — the whole list dissolves; hoist stray blocks
        // to the grandparent individually.
        for block in stray_blocks {
            caller_append(block);
        }
        EndResult::Empty
    } else {
        // Mix: some real listItems + some stray blocks. Produce the list for
        // the valid items and hoist the stray blocks.
        EndResult::WithHoists {
            node: json!({ "type": list_type, "content": list_items }),
            hoists: stray_blocks,
        }
    }
}

/// Shared task-list reclassification logic for both `BulletList` and
/// `OrderedList` containers.
///
/// When a list container (bullet OR ordered) contains at least one `taskItem`
/// child, the entire container is reclassified to a `taskList` — ordinal
/// numbering is dropped (lossy), but checkbox state is preserved and is
/// consistent with how bullet task lists are handled (BC-7.2.010 EC-ordered).
///
/// Decision rationale: ADF has no ordered task list node — `orderedList`
/// permits only `listItem` children and rejects `taskItem`, which causes a Jira
/// HTTP 400 on `orderedList > taskItem`. GFM's `1. [ ] x` renders as a
/// checkbox list on GitHub (ordinal is purely cosmetic for checked items).
/// Promoting to `taskList` preserves the user's checkbox intent and is
/// symmetric with the bullet-list rule, eliminating a bullet/ordered asymmetry.
///
/// When no `taskItem` children are present `None` is returned; the caller
/// falls through to its own plain-list construction.
///
/// # Segment ordering
///
/// Returns an `EndResult` that preserves source document order — task runs are
/// flushed into `taskList` nodes and non-task blocks are hoisted as siblings
/// (identical to the BulletList path). See the BulletList arm doc-comment for
/// shape examples.
fn reclassify_as_task_list(children: Vec<Value>) -> Option<EndResult> {
    let has_task_items = children
        .iter()
        .any(|c| c.get("type").and_then(Value::as_str) == Some("taskItem"));
    if !has_task_items {
        return None;
    }

    let mut segments: Vec<Segment> = Vec::new();
    for child in children {
        let ty = child.get("type").and_then(Value::as_str).unwrap_or("");
        match ty {
            "taskItem" => segments.push(Segment::Task(child)),
            "taskList" => segments.push(Segment::Task(child)),
            "listItem" => {
                // Plain item in a mixed list — promote to taskItem TODO.
                let inline_content = extract_inline_from_list_item_content(&child);
                let trimmed = trim_leading_trailing_hardbreaks(inline_content);
                let promoted = json!({
                    "type": "taskItem",
                    "attrs": { "localId": "", "state": "TODO" },
                    "content": trimmed,
                });
                if !is_empty_block_container(&promoted) {
                    segments.push(Segment::Task(promoted));
                }
                // Hoist non-paragraph blocks from the plain listItem.
                if let Some(blocks) = child.get("content").and_then(|c| c.as_array()) {
                    for block in blocks {
                        if block.get("type").and_then(Value::as_str) != Some("paragraph") {
                            segments.push(Segment::Hoist(block.clone()));
                        }
                    }
                }
            }
            _ => segments.push(Segment::Hoist(child)),
        }
    }

    let mut output_nodes: Vec<Value> = Vec::new();
    let mut current_task_run: Vec<Value> = Vec::new();

    let flush_task_run = |run: &mut Vec<Value>, out: &mut Vec<Value>| {
        if !run.is_empty() {
            let task_list = json!({
                "type": "taskList",
                "attrs": { "localId": "" },
                "content": std::mem::take(run),
            });
            out.push(task_list);
        }
    };

    for seg in segments {
        match seg {
            Segment::Task(node) => {
                // ADF tuple-lead rule: a `taskList`'s first child MUST be a
                // `taskItem` (taskList = (taskItem, (taskItem|taskList)*)). A
                // nested `taskList` segment may therefore only ATTACH to a run
                // that already has a leading `taskItem` — it can never START one.
                //
                // F6-P1 fix: when a bare `taskList` segment arrives while the
                // current run is empty (no leading taskItem yet), wrapping it in
                // `flush_task_run` would emit an invalid `taskList > taskList`
                // (first child is a taskList). Instead hoist it as a sibling
                // block — a nested taskList is itself a valid stand-alone block.
                // This surfaces from compositions like
                //   `- [ ] o\n  - p\n    - [ ] deep\n  - [ ] sib`
                // where a plain item's nested task-sublist hoists a lone
                // `taskList` ahead of the next taskItem run.
                let is_task_list = node.get("type").and_then(Value::as_str) == Some("taskList");
                if is_task_list && current_task_run.is_empty() {
                    output_nodes.push(node);
                } else {
                    current_task_run.push(node);
                }
            }
            Segment::Hoist(block) => {
                flush_task_run(&mut current_task_run, &mut output_nodes);
                output_nodes.push(block);
            }
        }
    }
    flush_task_run(&mut current_task_run, &mut output_nodes);

    Some(if output_nodes.is_empty() {
        EndResult::Empty
    } else if output_nodes.len() == 1 {
        EndResult::Single(
            output_nodes
                .into_iter()
                .next()
                .expect("len checked == 1 above"),
        )
    } else {
        let mut iter = output_nodes.into_iter();
        let first = iter.next().expect("len checked >= 2 above");
        EndResult::WithHoists {
            node: first,
            hoists: iter.collect(),
        }
    })
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
    const REQUIRES_CONTENT: [&str; 10] = [
        "blockquote",
        "panel",
        "heading",
        "listItem",
        "bulletList",
        "orderedList",
        "table",
        "tableRow",
        // taskList: minItems:1 (schema-required); empty taskList is invalid ADF.
        "taskList",
        // taskItem: see structurally-empty branch below for the extended check.
        "taskItem",
    ];
    let node_type = node.get("type").and_then(Value::as_str).unwrap_or("");
    if !REQUIRES_CONTENT.contains(&node_type) {
        return false;
    }
    let content = node.get("content").and_then(Value::as_array);
    let Some(c) = content else {
        return false;
    };
    // Standard structural-only check for all container types EXCEPT taskItem.
    // The 8 existing types retain `c.is_empty()` semantics unchanged — this
    // MUST NOT change (test_is_empty_block_container_membership at ~line 2753).
    if node_type != "taskItem" {
        return c.is_empty();
    }
    // taskItem extended emptiness branch (AC-009 / EC-8 deliberate product choice):
    // treat a taskItem as structurally empty when its content array is empty OR
    // contains ONLY whitespace-only text nodes and/or bare hardBreak nodes.
    // A lone hardBreak is schema-valid ADF but carries no semantic content as a
    // task-item body; pruning it is the correct UX behavior (BC-7.2.010 EC-8).
    // The structural membership alone is INSUFFICIENT for hardBreak-only items —
    // `content: [hardBreak]` has a non-empty array and would NOT be pruned without
    // this branch. This branch is SCOPED TO taskItem ONLY (see above comment).
    //
    // Backslash-escape artifact: pulldown-cmark 0.13.3 does NOT produce a `HardBreak`
    // event for `- [ ] \\\n` in a tight list; instead it emits `Text(Borrowed("\\"))`.
    // A lone backslash is the failed-escape residue that carries no semantic task
    // content (same deliberate product choice as the hardBreak-only prune). Extend the
    // "empty text" check to also treat text nodes containing only ASCII backslashes
    // (possibly with surrounding whitespace) as structurally empty.
    c.is_empty()
        || c.iter().all(|n| {
            let ty = n.get("type").and_then(Value::as_str).unwrap_or("");
            match ty {
                "hardBreak" => true,
                "text" => n
                    .get("text")
                    .and_then(Value::as_str)
                    .is_some_and(|t| t.trim_matches('\\').trim().is_empty()),
                _ => false,
            }
        })
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
fn normalize_list_item_content(children: Vec<Value>, depth: usize) -> Result<Vec<Value>, JrError> {
    if depth >= MAX_ADF_DEPTH {
        return Err(JrError::UserError(
            "markdown nesting too deep (max 256 levels)".to_string(),
        ));
    }
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
                out.extend(normalize_list_item_content(inner, depth + 1)?);
            }
            Some("taskList") => {
                // `listItem.content` does NOT permit `taskList` (BC-7.2.010
                // obligation #1 / EC-5). Unwrap: each taskItem's inline content
                // is wrapped in a `paragraph` to form a `listItem`, and all
                // resulting `listItem` nodes are collected into a new `bulletList`.
                // Valid ADF shape: listItem > [bulletList > [listItem > paragraph(…)]].
                //
                // F-PASS13-C1 fix: when a `taskItem` has a nested `taskList`
                // sibling (a multi-level nested task list inside a plain outer
                // item), the converted nested bullets MUST be nested INSIDE the
                // preceding `listItem`'s content — not appended as a sibling
                // `bulletList` into `converted_items`, which would produce the
                // invalid shape `bulletList > [listItem, bulletList]`.
                // EC-13 "sublist belongs to its owning item" ownership rule.
                //
                // Checkbox state (TODO/DONE) is dropped — documented lossiness EC-10(b).
                let task_items = child
                    .get("content")
                    .and_then(|c| c.as_array())
                    .cloned()
                    .unwrap_or_default();
                let mut converted_items: Vec<Value> = Vec::new();
                for ti in task_items {
                    if ti["type"] == "taskList" {
                        // Nested taskList: recursively normalize → produces zero
                        // or more block nodes (typically one bulletList). Each
                        // block must be nested INSIDE the last listItem in
                        // converted_items (EC-13 ownership), not appended as a
                        // sibling. If there is no preceding listItem (nested
                        // taskList appears first), create a placeholder empty-
                        // paragraph listItem to satisfy ADF constraints.
                        let inner_blocks = normalize_list_item_content(vec![ti], depth + 1)?;
                        for block in inner_blocks {
                            if let Some(last_li) = converted_items.last_mut() {
                                // Append the sub-bulletList to the last listItem's
                                // content so the shape is:
                                //   listItem > [paragraph(a), bulletList > [listItem(b)]]
                                if let Some(arr) =
                                    last_li.get_mut("content").and_then(|c| c.as_array_mut())
                                {
                                    arr.push(block);
                                }
                            } else {
                                // No preceding listItem — nest inside a placeholder.
                                let placeholder_li = json!({
                                    "type": "listItem",
                                    "content": [
                                        json!({ "type": "paragraph", "content": [] }),
                                        block
                                    ]
                                });
                                converted_items.push(placeholder_li);
                            }
                        }
                    } else {
                        // taskItem — extract its inline content and wrap in paragraph.
                        let inline_content = ti
                            .get("content")
                            .and_then(|c| c.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let para = json!({ "type": "paragraph", "content": inline_content });
                        converted_items.push(json!({ "type": "listItem", "content": [para] }));
                    }
                }
                if !converted_items.is_empty() {
                    out.push(json!({ "type": "bulletList", "content": converted_items }));
                }
            }
            Some("heading") => {
                let content = child.get("content").cloned().unwrap_or_else(|| json!([]));
                out.push(json!({ "type": "paragraph", "content": content }));
            }
            Some("table") => out.extend(flatten_table_to_paragraphs(&child)?),
            Some("rule") => { /* dropped — no content, invalid inside listItem */ }
            _ => out.push(child),
        }
    }
    Ok(out)
}

/// Normalize the children of a `blockquote` to the ADF-permitted content model.
///
/// ## What `blockquote.content` forbids
///
/// The ADF blockquote schema allows only `paragraph`, `heading`, `bulletList`,
/// `orderedList`, `codeBlock`, `rule`, `mediaSingle`, and `blockquote`.
/// Notably, **`table`**, `taskList`, `panel`, `taskItem`, and `listItem` are
/// all forbidden. The one forbidden type reachable from pulldown-cmark 0.13.3
/// is **`taskList`**: pulldown's task-marker scan is container-agnostic (the
/// `>` prefix is stripped on a prior loop iteration; the scan then runs
/// identically to the top-level case), so `> - [ ] item` emits
/// `blockquote > taskList`.
/// This normalization is therefore **required and unconditional** (#471,
/// BC-7.2.010 obligation #2 / EC-6).
///
/// ## Handled (produced by pulldown-cmark 0.13.3)
///
/// - **`taskList`** → unwrapped: each `taskItem`'s inline content is promoted
///   to a `paragraph` inside the blockquote. Checkbox state is dropped —
///   lossy (EC-10(c)). Nested `taskList` inside a blockquote-level `taskList`
///   is handled recursively.
///
/// ## Not expected from pulldown-cmark (no handling needed)
///
/// pulldown-cmark never emits `panel`, `table`, or another `blockquote` as a
/// direct child of `blockquote` — those are handled by `normalize_panel_content`
/// for the panel case and by event-stream ordering for nested blockquotes (inner
/// blockquotes finalize before the outer one, so `end(BlockQuote)` sees only
/// already-normalized content). No explicit guard is needed here.
///
/// All other node types pass through unchanged.
fn normalize_blockquote_content(children: Vec<Value>, depth: usize) -> Result<Vec<Value>, JrError> {
    if depth >= MAX_ADF_DEPTH {
        return Err(JrError::UserError(
            "markdown nesting too deep (max 256 levels)".to_string(),
        ));
    }
    let mut out: Vec<Value> = Vec::new();
    for child in children {
        if child.get("type").and_then(Value::as_str) == Some("taskList") {
            // Unwrap: promote each taskItem's inline content to a paragraph.
            let task_items = child
                .get("content")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            for ti in task_items {
                match ti.get("type").and_then(Value::as_str) {
                    Some("taskItem") => {
                        let inline_content = ti
                            .get("content")
                            .and_then(|c| c.as_array())
                            .cloned()
                            .unwrap_or_default();
                        out.push(json!({ "type": "paragraph", "content": inline_content }));
                    }
                    Some("taskList") => {
                        // Nested taskList inside the blockquote-level taskList:
                        // recurse to unwrap its items too.
                        out.extend(normalize_blockquote_content(vec![ti], depth + 1)?);
                    }
                    _ => {
                        // Unexpected node inside taskList — pass through defensively.
                        out.push(ti);
                    }
                }
            }
        } else {
            out.push(child);
        }
    }
    Ok(out)
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
fn normalize_panel_content(children: Vec<Value>, depth: usize) -> Result<Vec<Value>, JrError> {
    if depth >= MAX_ADF_DEPTH {
        return Err(JrError::UserError(
            "markdown nesting too deep (max 256 levels)".to_string(),
        ));
    }
    let mut out: Vec<Value> = Vec::new();
    for mut child in children {
        match child["type"].as_str() {
            Some("panel") | Some("blockquote") => {
                let inner = child
                    .get("content")
                    .and_then(|c| c.as_array())
                    .cloned()
                    .unwrap_or_default();
                out.extend(normalize_panel_content(inner, depth + 1)?);
            }
            Some("table") => out.extend(flatten_table_to_paragraphs(&child)?),
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
    Ok(out)
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
fn flatten_table_to_paragraphs(table: &Value) -> Result<Vec<Value>, JrError> {
    let mut paragraphs: Vec<Value> = Vec::new();
    let Some(rows) = table.get("content").and_then(|c| c.as_array()) else {
        return Ok(paragraphs);
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
                        let text = adf_to_text(&doc)?.trim_end().replace(['\n', '\r'], " ");
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
    Ok(paragraphs)
}

/// EC-16 inline-flattening for `taskItem`: strip paragraph wrappers from the
/// children of a `taskItem` (since `taskItem.content` is inline-only), concatenating
/// paragraphs with a `hardBreak` separator between them.
///
/// pulldown-cmark wraps item bodies in `Tag::Paragraph`, so a task item can have
/// multiple paragraph children if there are blank lines between them. We flatten:
///   [paragraph([text("line1")]), paragraph([text("line2")])]
///   → [text("line1"), hardBreak, text("line2")]
///
/// Non-paragraph children are silently skipped (defense-in-depth: both callers
/// pre-filter to paragraph-only, so this branch is unreachable under normal use;
/// skipping keeps `taskItem.content` inline-only rather than emitting invalid ADF or
/// panicking on user input). The caller then applies `trim_leading_trailing_hardbreaks`
/// to remove any leading/trailing hardBreak nodes introduced by this process.
fn flatten_task_item_to_inline(children: Vec<Value>) -> Vec<Value> {
    let mut result: Vec<Value> = Vec::new();
    let mut first = true;
    for child in children {
        if child.get("type").and_then(Value::as_str) == Some("paragraph") {
            // Extract inline nodes from the paragraph.
            let inline = child
                .get("content")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            // Only inject separator if the previous paragraph contributed content.
            // If `inline` is empty, we skip the separator (prevents double hardBreaks
            // adjacent to empty paragraphs; the trim pass handles boundary cases).
            if !inline.is_empty() {
                if !first {
                    result.push(json!({ "type": "hardBreak" }));
                }
                result.extend(inline);
                first = false;
            }
            // Empty paragraph: no separator emitted (trim pass handles trim near empties).
        } else {
            // SEC-002 / CWE-617: both callers pre-filter to paragraph-only (see CR-014),
            // so this branch is a contract violation. In debug builds, assert loudly so
            // developers catch a missed pre-filter immediately. In release builds, skip
            // the node gracefully — emitting a non-paragraph into taskItem.content would
            // produce invalid ADF (Jira 400), and panicking on user input is worse.
            debug_assert!(
                false,
                "non-paragraph passed to flatten_task_item_to_inline: {:?}",
                child.get("type")
            );
            // Graceful skip: preserve taskItem.content as inline-only (ADF invariant).
        }
    }
    result
}

/// Remove any leading and trailing `hardBreak` nodes from an inline-content array.
/// Also removes a hardBreak that was injected by `flatten_task_item_to_inline` but
/// is adjacent to an empty paragraph (those produce no inline nodes, so a separator
/// would be injected before the first real content of the next paragraph — trim it).
///
/// This implements the "general hardBreak trim rule" from BC-7.2.010 EC-16:
/// `taskItem.content` must NEVER begin or end with a `hardBreak`.
fn trim_leading_trailing_hardbreaks(mut content: Vec<Value>) -> Vec<Value> {
    let is_hb = |n: &Value| n.get("type").and_then(Value::as_str) == Some("hardBreak");
    // CR-015: single-pass leading trim via drain instead of repeated O(n) remove(0).
    let first_non_hb = content
        .iter()
        .position(|n| !is_hb(n))
        .unwrap_or(content.len());
    if first_non_hb > 0 {
        content.drain(..first_non_hb);
    }
    // Trailing trim: pop is already O(1); the while loop is fine since at most
    // the last few nodes are removed.
    while content.last().is_some_and(is_hb) {
        content.pop();
    }
    content
}

/// Extract inline content from a `listItem` node for EC-3 mixed-list promotion.
/// A `listItem` produced by `end(NodeKind::ListItem)` has its content wrapped in
/// `paragraph` nodes (via `wrap_inlines_as_blocks`). For promotion to `taskItem`
/// we need the raw inline content, not the paragraph wrappers.
fn extract_inline_from_list_item_content(list_item: &Value) -> Vec<Value> {
    let mut result: Vec<Value> = Vec::new();
    let Some(blocks) = list_item.get("content").and_then(|c| c.as_array()) else {
        return result;
    };
    for block in blocks {
        // CR-007: non-paragraph blocks (e.g. nested bulletList) cannot fit in
        // taskItem.content (inline-only) and are skipped — the caller's hoist
        // path is responsible for propagating them to the parent.
        if block.get("type").and_then(Value::as_str) == Some("paragraph") {
            if let Some(inline) = block.get("content").and_then(|c| c.as_array()) {
                result.extend(inline.iter().cloned());
            }
        }
    }
    result
}

/// Post-normalization, post-pruning DFS pre-order walk: assign monotonically
/// increasing 1-based counter strings (`"1"`, `"2"`, …) to all `taskList` and
/// `taskItem` nodes' `attrs.localId` fields. Container nodes are numbered before
/// their children (pre-order). Pruned nodes do not participate and do not consume
/// counter slots. No `uuid` crate dependency (BC-7.2.010 §Required attributes).
///
/// The counter is document-wide and unique across all taskList/taskItem nodes.
/// Called from `markdown_to_adf` after `finish()`, before `autolink_bare_urls`.
/// The ordering is immaterial to correctness (`autolink_bare_urls` only adds
/// `link` marks to text nodes and never adds or removes task-list nodes), but
/// the source order is: `finish()` → `assign_local_ids` → `autolink_bare_urls`.
fn assign_local_ids(nodes: &mut [Value]) -> Result<(), JrError> {
    let mut counter = 0u64;
    assign_local_ids_walk(nodes, &mut counter, 0)
}

fn assign_local_ids_walk(
    nodes: &mut [Value],
    counter: &mut u64,
    depth: usize,
) -> Result<(), JrError> {
    if depth >= MAX_ADF_DEPTH {
        return Err(JrError::UserError(
            "markdown nesting too deep (max 256 levels)".to_string(),
        ));
    }
    for node in nodes.iter_mut() {
        // CR-004: compare &str directly instead of allocating a String via to_owned().
        let node_type = node.get("type").and_then(Value::as_str).unwrap_or("");
        if node_type == "taskList" || node_type == "taskItem" {
            *counter += 1;
            if let Some(obj) = node.as_object_mut() {
                let attrs = obj.entry("attrs").or_insert_with(|| json!({}));
                if let Some(a) = attrs.as_object_mut() {
                    a.insert("localId".to_string(), json!(counter.to_string()));
                }
            }
        }
        // Recurse into content regardless of node type (task lists can be
        // nested inside panels, blockquotes, etc.; items at any depth need IDs).
        if let Some(content) = node.get_mut("content").and_then(Value::as_array_mut) {
            assign_local_ids_walk(content, counter, depth + 1)?;
        }
    }
    Ok(())
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

pub fn adf_to_text(adf: &Value) -> Result<String, JrError> {
    let mut r = AdfRenderer::new();
    r.render_doc(adf)?;
    Ok(r.finish())
}

struct AdfRenderer {
    output: String,
    list_stack: Vec<ListFrame>,
}

enum ListFrame {
    Bullet,
    Ordered { next_index: u64 },
    // GFM task list frame. Used for indentation tracking in nested task lists.
    // `adf_to_text` renders taskItem with `- [x] ` or `- [ ] ` prefix.
    Task,
}

impl AdfRenderer {
    fn new() -> Self {
        Self {
            output: String::new(),
            list_stack: Vec::new(),
        }
    }

    fn render_doc(&mut self, adf: &Value) -> Result<(), JrError> {
        if let Some(content) = adf.get("content").and_then(|c| c.as_array()) {
            for node in content {
                self.render_node(node, 0)?;
            }
        }
        Ok(())
    }

    fn render_node(&mut self, node: &Value, depth: usize) -> Result<(), JrError> {
        if depth >= MAX_ADF_DEPTH {
            return Err(JrError::UserError(
                "ADF response nesting too deep (max 256 levels) — the issue data returned by Jira cannot be rendered".to_string(),
            ));
        }
        let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match node_type {
            "text" => {
                let text = node.get("text").and_then(|t| t.as_str()).unwrap_or("");
                let marks = node.get("marks").and_then(|m| m.as_array());
                self.output.push_str(&apply_marks(text, marks));
            }
            "paragraph" => {
                self.render_children(node, depth)?;
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
                self.render_children(node, depth)?;
                self.output.push('\n');
            }
            "taskList" => {
                // Recurse into task list children using ListFrame::Task for
                // indentation tracking (2 spaces per nesting level, same as
                // bulletList / orderedList). (BC-7.2.010 reverse path; AC-010/012)
                self.list_stack.push(ListFrame::Task);
                self.render_children(node, depth)?;
                self.list_stack.pop();
            }
            "taskItem" => {
                // Render a task item with `- [x] ` (DONE) or `- [ ] ` (TODO/other).
                // Indentation: 2 spaces × (nesting depth - 1), matching the listItem arm.
                // The state comparison is case-insensitive (EC-12: external ADF may use
                // lowercase "done"). Inline content follows directly — no paragraph wrapper.
                let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));
                self.output.push_str(&indent);
                let state = node
                    .get("attrs")
                    .and_then(|a| a.get("state"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("");
                let prefix = if state.eq_ignore_ascii_case("DONE") {
                    "- [x] "
                } else {
                    "- [ ] "
                };
                self.output.push_str(prefix);
                // Render inline content directly (no paragraph wrapper in taskItem).
                if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                    for child in content {
                        self.render_node(child, depth + 1)?;
                    }
                }
                self.output.push('\n');
            }
            "bulletList" => {
                self.list_stack.push(ListFrame::Bullet);
                self.render_children(node, depth)?;
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
                self.render_children(node, depth)?;
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
                self.render_children(node, depth)?;
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
                self.render_children(node, depth)?;
                self.output.push_str("\n```\n");
            }
            "blockquote" => {
                let start = self.output.len();
                self.render_children(node, depth)?;

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
                self.render_children(node, depth)?;
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
                self.render_children(node, depth)?;
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
                    self.render_cell_inline(cell)?;
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
                self.render_cell_inline(node)?;
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
                    self.render_children(node, depth)?;
                }
            }
        }
        Ok(())
    }

    fn render_children(&mut self, node: &Value, depth: usize) -> Result<(), JrError> {
        if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
            for child in content {
                self.render_node(child, depth + 1)?;
            }
        }
        Ok(())
    }

    /// Render a tableCell/tableHeader's children in "flat" mode: a paragraph's
    /// inline content is emitted without its trailing newline (which would
    /// break the "| cell | cell |" row structure). Other block types inside
    /// a cell (rare but legal per the schema) fall back to normal rendering.
    fn render_cell_inline(&mut self, cell: &Value) -> Result<(), JrError> {
        let Some(content) = cell.get("content").and_then(|c| c.as_array()) else {
            return Ok(());
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
        Ok(())
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
            _ => {}
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
/// **Read-tolerance for externally-produced or legacy ADF (BC-7.2.015 EC-7):**
/// The write path (`AdfBuilder::push_code`) never emits typographic marks
/// alongside `code` — the allowlist filter (issue #571) strips them at
/// emission time. However, externally-produced or legacy ADF may contain
/// combinations such as `[strong, code]`. This function renders such nodes
/// tolerantly (e.g., `[strong, code]` → `` **`x`** ``). The code-innermost
/// behavior is intentional read-leniency and does NOT imply the write path
/// may produce it — the write and read paths are orthogonal by design.
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

    /// Assert that the JSON `marks` array contains exactly the mark type names
    /// in `expected`, treated as an unordered set (same length, same multiset
    /// of `"type"` string values). Panics with a formatted message including
    /// the actual mark-types vector on mismatch.
    ///
    /// AC-001 (BC-7.2.015 precondition: test helpers contract).
    fn assert_marks_eq(marks: &Value, expected: &[&str]) {
        let actual: Vec<&str> = match marks.as_array() {
            Some(arr) => arr.iter().filter_map(|m| m["type"].as_str()).collect(),
            None => vec![],
        };
        let mut actual_sorted = actual.clone();
        actual_sorted.sort_unstable();
        let mut expected_sorted = expected.to_vec();
        expected_sorted.sort_unstable();
        assert_eq!(
            actual_sorted, expected_sorted,
            "mark types mismatch — expected {expected:?}, got {actual:?}"
        );
    }

    /// Assert that the JSON `marks` array contains a mark of type `"link"`
    /// whose `attrs["href"]` field equals `expected_href` character-for-character.
    /// Uses field-by-field access — does NOT assert on `attrs["title"]`
    /// (absent for no-title links).
    ///
    /// AC-001 (BC-7.2.015 precondition: test helpers contract).
    fn assert_link_mark_with_href(marks: &Value, expected_href: &str) {
        let found = marks
            .as_array()
            .map(|arr| {
                arr.iter().any(|m| {
                    m["type"].as_str() == Some("link")
                        && m["attrs"]["href"].as_str() == Some(expected_href)
                })
            })
            .unwrap_or(false);
        assert!(
            found,
            "expected link mark with href={expected_href:?} in marks: {marks}"
        );
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
        assert_eq!(adf_to_text(&adf).unwrap(), "Hello world");
    }

    #[test]
    fn test_markdown_heading() {
        let adf = markdown_to_adf("## Root cause").unwrap();
        assert_eq!(adf["content"][0]["type"], "heading");
        assert_eq!(adf["content"][0]["attrs"]["level"], 2);
    }

    #[test]
    fn test_markdown_list() {
        let adf = markdown_to_adf("- item one\n- item two").unwrap();
        assert_eq!(adf["content"][0]["type"], "bulletList");
        let items = adf["content"][0]["content"].as_array().unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_markdown_code_block() {
        let adf = markdown_to_adf("```\nlet x = 1;\n```").unwrap();
        assert_eq!(adf["content"][0]["type"], "codeBlock");
    }

    #[test]
    fn test_adf_roundtrip_heading() {
        let adf = markdown_to_adf("## Title\nSome text").unwrap();
        let text = adf_to_text(&adf).unwrap();
        assert!(text.contains("## Title"));
        assert!(text.contains("Some text"));
    }

    #[test]
    fn test_render_unknown_leaf_drops_silently() {
        let adf = json!({
            "type": "doc",
            "content": [{ "type": "mediaGroup" }]
        });
        assert_eq!(adf_to_text(&adf).unwrap(), "");
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
        let text = adf_to_text(&adf).unwrap();
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
        let adf = markdown_to_adf(input).unwrap();
        insta::assert_json_snapshot!("markdown_complex_to_adf", adf);
    }

    #[test]
    fn test_markdown_ordered_list_sets_order_when_start_is_not_one() {
        let adf = markdown_to_adf("5. first\n6. second").unwrap();
        assert_eq!(adf["content"][0]["type"], "orderedList");
        assert_eq!(adf["content"][0]["attrs"]["order"], 5);
        assert_eq!(adf["content"][0]["content"][0]["type"], "listItem");
    }

    #[test]
    fn test_markdown_ordered_list_omits_order_when_start_is_one() {
        let adf = markdown_to_adf("1. alpha\n2. beta").unwrap();
        assert_eq!(adf["content"][0]["type"], "orderedList");
        assert!(adf["content"][0]["attrs"].is_null());
    }

    // --- Ordered list + task markers (EC-ordered, fix for invalid orderedList > taskItem) ---

    #[test]
    fn test_markdown_ordered_task_list_produces_task_list_not_ordered_list() {
        // `1. [ ] a\n2. [x] b` must reclassify to taskList, NOT orderedList.
        // orderedList.content only permits listItem; taskItem would be Jira 400.
        let adf = markdown_to_adf("1. [ ] a\n2. [x] b\n").unwrap();
        let top = &adf["content"][0];
        assert_eq!(
            top["type"], "taskList",
            "ordered list with task markers must become taskList, got: {adf}"
        );
        let items = top["content"]
            .as_array()
            .expect("taskList must have content");
        assert_eq!(items.len(), 2, "expected 2 taskItems, got: {adf}");
        assert_eq!(items[0]["type"], "taskItem");
        assert_eq!(items[0]["attrs"]["state"], "TODO");
        assert_eq!(
            items[0]["content"][0]["text"], "a",
            "first taskItem text: {adf}"
        );
        assert_eq!(items[1]["type"], "taskItem");
        assert_eq!(items[1]["attrs"]["state"], "DONE");
        assert_eq!(
            items[1]["content"][0]["text"], "b",
            "second taskItem text: {adf}"
        );
        // ADF structural validity
        assert_valid_adf_structure(&adf);
    }

    #[test]
    fn test_markdown_ordered_task_list_mixed_promotes_plain_to_todo() {
        // `1. [ ] a\n2. plain` — plain item must be promoted to taskItem TODO.
        let adf = markdown_to_adf("1. [ ] a\n2. plain\n").unwrap();
        let top = &adf["content"][0];
        assert_eq!(
            top["type"], "taskList",
            "mixed ordered list must become taskList: {adf}"
        );
        let items = top["content"]
            .as_array()
            .expect("taskList must have content");
        assert_eq!(items.len(), 2, "expected 2 taskItems (promoted): {adf}");
        assert_eq!(items[0]["attrs"]["state"], "TODO");
        assert_eq!(items[1]["attrs"]["state"], "TODO");
        assert_valid_adf_structure(&adf);
    }

    #[test]
    fn test_markdown_ordered_task_list_nested_produces_nested_task_list() {
        // `1. [ ] a\n   1. [ ] b` — nested ordered task list per EC-13.
        let adf = markdown_to_adf("1. [ ] a\n   1. [ ] b\n").unwrap();
        // The outer container must be a taskList.
        let outer = &adf["content"][0];
        assert_eq!(outer["type"], "taskList", "outer must be taskList: {adf}");
        // Structural validity covers the nested shape.
        assert_valid_adf_structure(&adf);
    }

    #[test]
    fn test_markdown_plain_ordered_list_unchanged_without_task_markers() {
        // Plain `1. first\n2. second` (no task markers) must remain orderedList.
        let adf = markdown_to_adf("1. first\n2. second\n").unwrap();
        let top = &adf["content"][0];
        assert_eq!(
            top["type"], "orderedList",
            "plain ordered list must stay orderedList: {adf}"
        );
        let items = top["content"]
            .as_array()
            .expect("orderedList must have content");
        assert_eq!(items.len(), 2);
        for item in items {
            assert_eq!(
                item["type"], "listItem",
                "orderedList children must be listItem: {adf}"
            );
        }
        assert_valid_adf_structure(&adf);
    }

    #[test]
    fn test_markdown_hard_break() {
        let adf = markdown_to_adf("line one  \nline two").unwrap();
        let para = &adf["content"][0];
        assert_eq!(para["type"], "paragraph");
        let contents = para["content"].as_array().unwrap();
        assert!(contents.iter().any(|n| n["type"] == "hardBreak"));
    }

    #[test]
    fn test_markdown_horizontal_rule() {
        let adf = markdown_to_adf("above\n\n---\n\nbelow").unwrap();
        let has_rule = adf["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["type"] == "rule");
        assert!(has_rule, "expected a rule node, got: {adf}");
    }

    #[test]
    fn test_markdown_soft_break_becomes_space() {
        let adf = markdown_to_adf("first line\nsecond line").unwrap();
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
        let adf = markdown_to_adf("- outer\n  - inner").unwrap();
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
        let adf = markdown_to_adf("> quoted text").unwrap();
        let bq = &adf["content"][0];
        assert_eq!(bq["type"], "blockquote");
        let para = &bq["content"][0];
        assert_eq!(para["type"], "paragraph");
        assert_eq!(para["content"][0]["text"], "quoted text");
    }

    #[test]
    fn test_markdown_code_block_with_language() {
        let adf = markdown_to_adf("```rust\nfn x() {}\n```").unwrap();
        let block = &adf["content"][0];
        assert_eq!(block["type"], "codeBlock");
        assert_eq!(block["attrs"]["language"], "rust");
        assert_eq!(block["content"][0]["text"], "fn x() {}\n");
    }

    #[test]
    fn test_markdown_empty_input() {
        let adf = markdown_to_adf("").unwrap();
        assert_eq!(adf["type"], "doc");
        assert_eq!(adf["content"], json!([]));
    }

    #[test]
    fn test_markdown_inline_code_mark_and_composition() {
        // Plain inline code: emits text with a `code` mark.
        let adf = markdown_to_adf("see `foo` here").unwrap();
        let code_node = adf["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "foo")
            .expect("expected a text node for 'foo'");
        assert_eq!(code_node["marks"][0]["type"], "code");

        // Inline code inside bold: post-fix, the push_code allowlist filter strips
        // `strong` from the code node's marks — only `code` remains.
        // AC-002 (BC-7.2.015 EC-1 regression pin; BC-7.2.007 EC-2 pre-#571 write-strict
        // clause). Pre-fix this assertion FAILS (old code emits strong+code on the node).
        let adf = markdown_to_adf("**bold `code` bold**").unwrap();
        let code_node = adf["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "code")
            .expect("expected a text node for 'code'");
        assert_marks_eq(&code_node["marks"], &["code"]);
    }

    // ── BC-7.2.015 test suite ─────────────────────────────────────────────────
    // Control + EC-1..EC-6 + PANEL-ANCHOR anchors (VP-571-002).
    // All "stripped" tests are RED pre-fix (push_code filter not yet applied)
    // and GREEN post-fix. EC-5 (link preserved) and the control are GREEN both
    // pre-fix and post-fix.
    // ──────────────────────────────────────────────────────────────────────────

    /// Control/baseline anchor (BC-7.2.015 CONTROL): bare inline code with no
    /// surrounding marks produces exactly `[code]`. GREEN pre-fix AND post-fix —
    /// validates the assertion harness and the plain-code path in push_code.
    #[test]
    fn test_bc_7_2_015_plain_code_baseline() {
        let adf = markdown_to_adf("`x`").unwrap();
        let code_node = adf["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "x")
            .expect("expected text node 'x'");
        assert_marks_eq(&code_node["marks"], &["code"]);
    }

    /// EC-1 (BC-7.2.015): `strong` wrapping inline code — `strong` must be
    /// stripped from the code text node. RED pre-fix, GREEN post-fix.
    #[test]
    fn test_bc_7_2_015_strong_stripped_from_code_node() {
        let adf = markdown_to_adf("**`x`**").unwrap();
        let code_node = adf["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "x")
            .expect("expected text node 'x'");
        assert_marks_eq(&code_node["marks"], &["code"]);
    }

    /// EC-2 (BC-7.2.015): `em` wrapping inline code — `em` must be stripped.
    /// Pre-fix RED/GREEN status empirically resolved by Task 2 observation.
    #[test]
    fn test_bc_7_2_015_em_stripped_from_code_node() {
        let adf = markdown_to_adf("_`x`_").unwrap();
        let code_node = adf["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "x")
            .expect("expected text node 'x'");
        assert_marks_eq(&code_node["marks"], &["code"]);
    }

    /// EC-3 (BC-7.2.015): `strike` wrapping inline code — `strike` must be
    /// stripped. Pre-fix RED/GREEN status empirically resolved by Task 2.
    #[test]
    fn test_bc_7_2_015_strike_stripped_from_code_node() {
        let adf = markdown_to_adf("~~`x`~~").unwrap();
        let code_node = adf["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "x")
            .expect("expected text node 'x'");
        assert_marks_eq(&code_node["marks"], &["code"]);
    }

    /// EC-4 (BC-7.2.015): `subsup` wrapping inline code — `subsup` must be
    /// stripped. Primary regression target; closes BC-7.2.007 EC-2 follow-up
    /// (issue #474 → #571). Pre-fix RED/GREEN status empirically resolved by
    /// Task 2 observation.
    #[test]
    fn test_bc_7_2_015_subsup_stripped_from_code_node() {
        let adf = markdown_to_adf("^`x`^").unwrap();
        let code_node = adf["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "x")
            .expect("expected text node 'x'");
        assert_marks_eq(&code_node["marks"], &["code"]);
    }

    /// EC-5 (BC-7.2.015): `link` coexisting with inline code — `link` MUST be
    /// preserved (schema-valid co-mark). GREEN pre-fix AND post-fix.
    /// Two-part assertion: mark-types set AND attrs["href"] field-by-field check.
    #[test]
    fn test_bc_7_2_015_link_preserved_on_code_node() {
        let adf = markdown_to_adf("[`x`](https://ex/)").unwrap();
        let code_node = adf["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "x")
            .expect("expected text node 'x'");
        // Both `code` and `link` marks must be present (unordered set).
        assert_marks_eq(&code_node["marks"], &["code", "link"]);
        // The link mark's attrs["href"] must be preserved verbatim.
        assert_link_mark_with_href(&code_node["marks"], "https://ex/");
    }

    /// EC-6 / VP-571-003 (BC-7.2.015): mixed-range — code node strips `strong`,
    /// surrounding non-code text nodes RETAIN `strong`. Catches the
    /// "filter active_marks in-place" mutation. Pre-fix code-node RED, surrounds GREEN.
    #[test]
    fn test_bc_7_2_015_mixed_range_surrounding_marks_retained() {
        let adf = markdown_to_adf("**a `b` c**").unwrap();
        let para_content = adf["content"][0]["content"].as_array().unwrap();

        // "a " — must keep strong.
        let a_node = para_content
            .iter()
            .find(|n| n["text"] == "a ")
            .expect("expected text node 'a '");
        assert_marks_eq(&a_node["marks"], &["strong"]);

        // "b" — code node; strong must be stripped.
        let b_node = para_content
            .iter()
            .find(|n| n["text"] == "b")
            .expect("expected text node 'b'");
        assert_marks_eq(&b_node["marks"], &["code"]);

        // " c" — must keep strong.
        let c_node = para_content
            .iter()
            .find(|n| n["text"] == " c")
            .expect("expected text node ' c'");
        assert_marks_eq(&c_node["marks"], &["strong"]);
    }

    /// VP-571-003 (BC-7.2.015): multi-mark wrapper — code node strips BOTH `em`
    /// and `strong`; all sibling text nodes retain their full mark stack.
    #[test]
    fn test_bc_7_2_015_multi_mark_wrapper_only_code_node_stripped() {
        // _a **b `c` d** e_ → five text nodes; only "c" loses its marks.
        let adf = markdown_to_adf("_a **b `c` d** e_").unwrap();
        let para_content = adf["content"][0]["content"].as_array().unwrap();

        // "c" — code node; BOTH em and strong stripped.
        let c_node = para_content
            .iter()
            .find(|n| n["text"] == "c")
            .expect("expected text node 'c'");
        assert_marks_eq(&c_node["marks"], &["code"]);

        // "a " — only em (not yet inside the strong span).
        let a_node = para_content
            .iter()
            .find(|n| n["text"] == "a ")
            .expect("expected text node 'a '");
        assert_marks_eq(&a_node["marks"], &["em"]);

        // "b " — em + strong.
        let b_node = para_content
            .iter()
            .find(|n| n["text"] == "b ")
            .expect("expected text node 'b '");
        assert_marks_eq(&b_node["marks"], &["em", "strong"]);

        // " d" — em + strong.
        let d_node = para_content
            .iter()
            .find(|n| n["text"] == " d")
            .expect("expected text node ' d'");
        assert_marks_eq(&d_node["marks"], &["em", "strong"]);

        // " e" — only em (outside the strong span).
        let e_node = para_content
            .iter()
            .find(|n| n["text"] == " e")
            .expect("expected text node ' e'");
        assert_marks_eq(&e_node["marks"], &["em"]);
    }

    /// PANEL-ANCHOR (VP-571-002 supplementary anchor): `strong`+code inside a
    /// GFM NOTE alert — `strong` must be stripped from the code text node even
    /// inside `panel.content`. Pre-fix RED expected by class-transfer from EC-1.
    #[test]
    fn test_bc_7_2_015_alert_wrapper_strong_code_stripped() {
        let adf = markdown_to_adf("> [!NOTE]\n> **`x`**").unwrap();

        // Top-level node must be a panel with panelType "info".
        let panel = &adf["content"][0];
        assert_eq!(panel["type"], "panel", "expected panel at doc root: {adf}");
        assert_eq!(
            panel["attrs"]["panelType"], "info",
            "expected panelType info: {adf}"
        );

        // Within panel.content, the paragraph holds the text node "x".
        let panel_para = &panel["content"][0];
        assert_eq!(
            panel_para["type"], "paragraph",
            "expected paragraph inside panel: {panel}"
        );
        let code_node = panel_para["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "x")
            .expect("expected text node 'x' inside panel paragraph");

        // strong must be stripped; only code remains.
        assert_marks_eq(&code_node["marks"], &["code"]);
    }

    // ── end BC-7.2.015 unit tests ─────────────────────────────────────────────

    /// Recursively walk an ADF node tree and assert that no text node whose
    /// `marks` array contains `{"type":"code"}` also carries any mark type
    /// outside the allow-set `{"code", "link", "annotation"}`.
    ///
    /// This is the BC-7.2.015 positive invariant expressed as a universal
    /// quantifier over the emitted ADF document: used by
    /// `prop_bc_7_2_015_no_code_marked_text_node_carries_typographic_marks`
    /// (VP-571-001) and by the integration tests in
    /// `tests/adf_code_mark_exclusivity.rs` (H-NEW-ADF-010 Calls A–D).
    ///
    /// Uses a generic recursive descent via the `content` array — any new
    /// container node type added to the ADF schema is covered automatically.
    /// Mark objects are flat (`{"type": "...", "attrs": {...}?}`) and are
    /// inspected directly; they do not require further recursion.
    fn assert_code_mark_exclusivity(node: &Value) {
        // Inspect this node's marks if present.
        if let Some(marks_arr) = node.get("marks").and_then(|m| m.as_array()) {
            let has_code = marks_arr.iter().any(|m| m["type"].as_str() == Some("code"));
            if has_code {
                const FORBIDDEN: &[&str] = &[
                    "strong",
                    "em",
                    "strike",
                    "subsup",
                    "underline",
                    "textColor",
                    "backgroundColor",
                ];
                for mark in marks_arr {
                    if let Some(t) = mark["type"].as_str() {
                        assert!(
                            !FORBIDDEN.contains(&t),
                            "VP-571-001: text node with `code` mark carries forbidden \
                             typographic mark '{t}'. Full marks: {marks_arr:?}. Node: {node}"
                        );
                    }
                }
            }
        }
        // Recurse into content array (generic descent — covers paragraph, heading,
        // blockquote, listItem, bulletList, orderedList, taskList, taskItem,
        // panel, table, tableRow, tableCell, tableHeader, and any future type).
        if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
            for child in content {
                assert_code_mark_exclusivity(child);
            }
        }
    }

    #[test]
    fn test_markdown_blockquote_inside_list_item_is_unwrapped_to_paragraph() {
        // `- > quoted` → pulldown-cmark emits blockquote inside Item. The ADF
        // `listItem` content model does NOT permit `blockquote` (only paragraph,
        // bulletList, orderedList, codeBlock, mediaSingle — see
        // docs/specs/adf-listitem-content-model.md, issue #470). We unwrap the
        // blockquote and splice its child paragraph(s) directly into the listItem.
        let adf = markdown_to_adf("- > quoted text").unwrap();
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
        let adf = markdown_to_adf("- # Heading text").unwrap();
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
        let adf = markdown_to_adf("- ### deep **bold** head").unwrap();
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
        let adf = markdown_to_adf("- > # quoted heading").unwrap();
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
        let adf = markdown_to_adf("- item\n\n  ---").unwrap();
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
        let adf = markdown_to_adf("- intro\n\n  | **a** | b |\n  | - | - |\n  | 1 | 2 |").unwrap();
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
        let paras = flatten_table_to_paragraphs(&table).unwrap();
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
        let adf = markdown_to_adf("-   \n\n    ---").unwrap();
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
        let adf = markdown_to_adf("- ```\n  let x = 1;\n  ```").unwrap();
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
        let adf = markdown_to_adf("- outer\n  1. a\n  2. b").unwrap();
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
        let adf = markdown_to_adf("before <span>x</span> after").unwrap();
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

    /// Bare-URL autolinking (#473). Scope: explicit-scheme `http(s)://` only.
    /// Find the link `href` on the first text node carrying a `link` mark whose
    /// text equals `expected_text`, searching a paragraph's inline content.
    fn link_href_for_text<'a>(para: &'a Value, expected_text: &str) -> Option<&'a str> {
        para["content"].as_array()?.iter().find_map(|n| {
            if n.get("text").and_then(Value::as_str) != Some(expected_text) {
                return None;
            }
            n.get("marks")?
                .as_array()?
                .iter()
                .find(|m| m.get("type").and_then(Value::as_str) == Some("link"))
                .and_then(|m| m.get("attrs"))
                .and_then(|a| a.get("href"))
                .and_then(Value::as_str)
        })
    }

    #[test]
    fn test_bare_https_url_becomes_link_mark() {
        let adf = markdown_to_adf("see https://example.com now").unwrap();
        let para = &adf["content"][0];
        assert_eq!(para["type"], "paragraph");
        // Surrounding text stays plain; the URL span carries a link mark.
        assert_eq!(
            link_href_for_text(para, "https://example.com"),
            Some("https://example.com"),
            "bare https URL must get a link mark: {adf}"
        );
        // The leading/trailing words remain unmarked plain text.
        let texts: Vec<&str> = para["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["text"].as_str())
            .collect();
        assert_eq!(texts, vec!["see ", "https://example.com", " now"], "{adf}");
    }

    #[test]
    fn test_bare_http_url_becomes_link_mark() {
        let adf = markdown_to_adf("http://a.co/x?q=1").unwrap();
        let para = &adf["content"][0];
        assert_eq!(
            link_href_for_text(para, "http://a.co/x?q=1"),
            Some("http://a.co/x?q=1"),
            "bare http URL must get a link mark: {adf}"
        );
    }

    #[test]
    fn test_bare_url_trailing_period_is_trimmed() {
        let adf = markdown_to_adf("visit https://example.com.").unwrap();
        let para = &adf["content"][0];
        // Trailing sentence period is NOT part of the link.
        assert_eq!(
            link_href_for_text(para, "https://example.com"),
            Some("https://example.com"),
            "trailing period must be excluded from the URL: {adf}"
        );
        let joined: String = para["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["text"].as_str())
            .collect();
        assert_eq!(joined, "visit https://example.com.", "no text lost: {adf}");
    }

    #[test]
    fn test_bare_url_wrapping_paren_not_captured() {
        let adf = markdown_to_adf("(https://example.com)").unwrap();
        let para = &adf["content"][0];
        assert_eq!(
            link_href_for_text(para, "https://example.com"),
            Some("https://example.com"),
            "wrapping close-paren must not be part of the URL: {adf}"
        );
    }

    #[test]
    fn test_bare_url_balanced_inner_parens_kept() {
        let adf = markdown_to_adf("https://en.wikipedia.org/wiki/Foo_(bar)").unwrap();
        let para = &adf["content"][0];
        assert_eq!(
            link_href_for_text(para, "https://en.wikipedia.org/wiki/Foo_(bar)"),
            Some("https://en.wikipedia.org/wiki/Foo_(bar)"),
            "balanced trailing parens are part of the URL: {adf}"
        );
    }

    #[test]
    fn test_url_in_inline_code_not_linkified() {
        let adf = markdown_to_adf("`https://example.com`").unwrap();
        let node = &adf["content"][0]["content"][0];
        let marks: Vec<&str> = node["marks"]
            .as_array()
            .map(|a| a.iter().filter_map(|m| m["type"].as_str()).collect())
            .unwrap_or_default();
        assert!(marks.contains(&"code"), "must stay code: {adf}");
        assert!(
            !marks.contains(&"link"),
            "code span must NOT be linkified: {adf}"
        );
    }

    #[test]
    fn test_url_in_code_block_not_linkified() {
        let adf = markdown_to_adf("```\nhttps://example.com\n```").unwrap();
        assert_eq!(adf["content"][0]["type"], "codeBlock", "{adf}");
        let node = &adf["content"][0]["content"][0];
        assert!(
            node.get("marks").is_none(),
            "code block content must NOT be linkified: {adf}"
        );
    }

    #[test]
    fn test_existing_markdown_link_not_double_linkified() {
        let adf = markdown_to_adf("[x](https://example.com)").unwrap();
        let content = adf["content"][0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1, "exactly one text node: {adf}");
        assert_eq!(
            content[0]["text"], "x",
            "link text preserved, not the URL: {adf}"
        );
        let link_marks = content[0]["marks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["type"] == "link")
            .count();
        assert_eq!(link_marks, 1, "must not stack a second link mark: {adf}");
    }

    #[test]
    fn test_www_url_stays_plain_text() {
        let adf = markdown_to_adf("see www.example.com here").unwrap();
        let para = &adf["content"][0];
        // www. is deliberately out of scope (no scheme to infer); stays plain.
        assert!(
            !contains_node_type(&adf, "link"),
            "www. URL must NOT be linkified (out of scope): {adf}"
        );
        let joined: String = para["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["text"].as_str())
            .collect();
        assert_eq!(joined, "see www.example.com here", "{adf}");
    }

    #[test]
    fn test_bare_email_stays_plain_text() {
        let adf = markdown_to_adf("ping user@example.com please").unwrap();
        assert!(
            !contains_node_type(&adf, "link"),
            "bare email must NOT be linkified (out of scope): {adf}"
        );
        let joined: String = adf["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["text"].as_str())
            .collect();
        assert_eq!(
            joined, "ping user@example.com please",
            "email text preserved: {adf}"
        );
    }

    #[test]
    fn test_uppercase_https_scheme_is_linkified() {
        // RFC 3986 / GFM treat schemes case-insensitively. The href preserves the
        // user's original case (we do not normalize the scheme or path).
        let adf = markdown_to_adf("see HTTPS://example.com now").unwrap();
        let para = &adf["content"][0];
        assert_eq!(
            link_href_for_text(para, "HTTPS://example.com"),
            Some("HTTPS://example.com"),
            "uppercase HTTPS scheme must be linkified, href preserves case: {adf}"
        );
    }

    #[test]
    fn test_mixed_case_scheme_is_linkified_and_path_case_preserved() {
        let adf = markdown_to_adf("Http://Example.com/Path").unwrap();
        let para = &adf["content"][0];
        assert_eq!(
            link_href_for_text(para, "Http://Example.com/Path"),
            Some("Http://Example.com/Path"),
            "mixed-case scheme linkified; path case preserved verbatim: {adf}"
        );
    }

    #[test]
    fn test_trailing_uppercase_in_scheme_is_linkified() {
        // `httpS://` matches `http` then the case-insensitive `https://` check.
        let adf = markdown_to_adf("httpS://example.com").unwrap();
        let para = &adf["content"][0];
        assert_eq!(
            link_href_for_text(para, "httpS://example.com"),
            Some("httpS://example.com"),
            "partial-uppercase scheme must still match: {adf}"
        );
    }

    #[test]
    fn test_bare_url_after_open_bracket_stays_plain_text() {
        // Deviation #1: GFM's autolink "before" set admits `[`, but we omit it to
        // avoid linking the inner URL of an unresolved reference shortcut. pulldown
        // emits `[https://example.com]` as literal text; the `[` before `http`
        // fails our boundary check, so no link is produced.
        let adf = markdown_to_adf("[https://example.com]").unwrap();
        assert!(
            !contains_node_type(&adf, "link"),
            "URL after `[` (reference-shortcut form) must stay plain text: {adf}"
        );
    }

    #[test]
    fn test_url_tight_against_preceding_word_not_matched() {
        // GFM boundary: an autolink must start at line-start, after whitespace, or
        // after one of *_~( . A scheme tight against a word char is not an autolink.
        let adf = markdown_to_adf("foohttps://example.com").unwrap();
        assert!(
            !contains_node_type(&adf, "link"),
            "URL tight against a preceding word char must NOT match: {adf}"
        );
    }

    #[test]
    fn test_bare_url_round_trips_to_markdown_link_text() {
        // A bare URL becomes a real link, so adf_to_text renders it in `[t](href)`
        // form — semantically correct (it IS a link now), not the bare string.
        let adf = markdown_to_adf("https://example.com").unwrap();
        let text = adf_to_text(&adf).unwrap();
        assert_eq!(
            text, "[https://example.com](https://example.com)",
            "bare URL round-trips as a markdown link: {text:?}"
        );
    }

    // --- #473 F5 adversary remediation: characterize live autolink paths ---

    #[test]
    fn test_bare_url_split_by_emphasis_links_only_leading_run() {
        // KNOWN LIMITATION (post-finish approach): inline markup inside a URL is
        // parsed FIRST, so `*b*` splits the URL into separate text nodes. The
        // autolink pass sees only the leading plain run and links that; the
        // emphasized tail is NOT part of the href. Documented in the spec's
        // "Deviations from GFM" section. This test pins the limitation so it is a
        // declared behavior, not an accident.
        let adf = markdown_to_adf("see https://example.com/a*b*c done").unwrap();
        let para = &adf["content"][0];
        assert_eq!(
            link_href_for_text(para, "https://example.com/a"),
            Some("https://example.com/a"),
            "only the leading run before the emphasis is linked: {adf}"
        );
        // The emphasized `b` is a separate em-marked node, not in the href.
        let b = para["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "b")
            .expect("emphasized 'b' node present");
        assert_eq!(
            b["marks"][0]["type"], "em",
            "tail keeps em, not link: {adf}"
        );
    }

    #[test]
    fn test_bare_url_inside_emphasis_keeps_em_and_link() {
        // A URL wholly inside an emphasis span arrives carrying an `em` mark; the
        // split must preserve it AND add `link` (two distinct mark types, valid).
        let adf = markdown_to_adf("*https://example.com*").unwrap();
        let node = &adf["content"][0]["content"][0];
        let mark_types: Vec<&str> = node["marks"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m["type"].as_str())
            .collect();
        assert!(mark_types.contains(&"em"), "em preserved: {adf}");
        assert!(mark_types.contains(&"link"), "link added: {adf}");
        // No duplicate-type marks (ADF treats marks as a type-keyed set).
        assert_eq!(
            mark_types.len(),
            2,
            "exactly em + link, no duplicates: {adf}"
        );
    }

    #[test]
    fn test_two_bare_urls_in_one_text_node_both_link() {
        // find_bare_url_spans returns multiple spans; split_text_node_on_urls
        // loops over them. Pins the cursor bookkeeping for >1 URL in one node.
        let adf = markdown_to_adf("https://a.example.com and https://b.example.com").unwrap();
        let para = &adf["content"][0];
        assert_eq!(
            link_href_for_text(para, "https://a.example.com"),
            Some("https://a.example.com"),
            "first URL linked: {adf}"
        );
        assert_eq!(
            link_href_for_text(para, "https://b.example.com"),
            Some("https://b.example.com"),
            "second URL linked: {adf}"
        );
        // The separator text between them stays plain.
        let joined: String = para["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["text"].as_str())
            .collect();
        assert_eq!(
            joined, "https://a.example.com and https://b.example.com",
            "no text lost across two URLs: {adf}"
        );
    }

    #[test]
    fn test_bare_url_with_port_is_preserved() {
        // The `:` trailing-trim rule must NOT strip a port (`:8080` is followed by
        // digits, so `:` is interior, not trailing). Load-bearing: pins port survival.
        let adf = markdown_to_adf("https://example.com:8080/path").unwrap();
        let para = &adf["content"][0];
        assert_eq!(
            link_href_for_text(para, "https://example.com:8080/path"),
            Some("https://example.com:8080/path"),
            "port must survive the trailing-colon trim rule: {adf}"
        );
    }

    #[test]
    fn test_bare_url_trailing_colon_is_trimmed() {
        // A genuinely trailing `:` (end of the run) IS trimmed, per GFM.
        let adf = markdown_to_adf("see https://example.com: next").unwrap();
        let para = &adf["content"][0];
        assert_eq!(
            link_href_for_text(para, "https://example.com"),
            Some("https://example.com"),
            "trailing colon excluded from URL: {adf}"
        );
    }

    #[test]
    fn test_bare_url_in_panel_is_linkified() {
        // The walker recurses into a `panel` AFTER normalize_panel_content ran.
        // A `link` is an inline text-node mark (not a node-level mark), so it does
        // not violate the panel "no node marks" rule. Pins the post-normalization path.
        let adf = markdown_to_adf("> [!NOTE]\n> see https://example.com here").unwrap();
        assert_eq!(adf["content"][0]["type"], "panel", "is a panel: {adf}");
        assert!(
            contains_node_type(&adf, "link"),
            "URL inside a panel must be linkified: {adf}"
        );
    }

    #[test]
    fn test_bare_url_in_table_cell_is_linkified() {
        let adf = markdown_to_adf("| a |\n|---|\n| https://example.com |").unwrap();
        assert!(contains_node_type(&adf, "table"), "is a table: {adf}");
        assert!(
            contains_node_type(&adf, "link"),
            "URL inside a table cell must be linkified: {adf}"
        );
    }

    #[test]
    fn test_markdown_italic_to_em_mark() {
        let adf = markdown_to_adf("*italic words*").unwrap();
        let text_node = &adf["content"][0]["content"][0];
        assert_eq!(text_node["type"], "text");
        assert_eq!(text_node["text"], "italic words");
        assert_eq!(text_node["marks"][0]["type"], "em");
    }

    #[test]
    fn test_markdown_bold_to_strong_mark() {
        let adf = markdown_to_adf("**bold words**").unwrap();
        let text_node = &adf["content"][0]["content"][0];
        assert_eq!(text_node["text"], "bold words");
        assert_eq!(text_node["marks"][0]["type"], "strong");
    }

    #[test]
    fn test_markdown_strikethrough_to_strike_mark() {
        let adf = markdown_to_adf("~~gone~~").unwrap();
        let text_node = &adf["content"][0]["content"][0];
        assert_eq!(text_node["text"], "gone");
        assert_eq!(text_node["marks"][0]["type"], "strike");
    }

    #[test]
    fn test_markdown_link_preserves_href_and_no_title() {
        let adf = markdown_to_adf("[jr](https://example.com/jr)").unwrap();
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
        let adf = markdown_to_adf(r#"[jr](https://example.com/jr "JR docs")"#).unwrap();
        let mark = &adf["content"][0]["content"][0]["marks"][0];
        assert_eq!(mark["type"], "link");
        assert_eq!(mark["attrs"]["href"], "https://example.com/jr");
        assert_eq!(mark["attrs"]["title"], "JR docs");
    }

    #[test]
    fn test_markdown_mixed_marks() {
        let adf = markdown_to_adf("**bold _italic_ bold**").unwrap();
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
        let adf = markdown_to_adf(r"\*not italic\*").unwrap();
        let text_node = &adf["content"][0]["content"][0];
        assert_eq!(text_node["text"], "*not italic*");
        // No em mark because backslash escapes the asterisks.
        assert!(text_node["marks"].is_null());
    }

    #[test]
    fn test_markdown_table_cells_and_headers() {
        let input = "| foo | bar |\n| --- | --- |\n| baz | qux |";
        let adf = markdown_to_adf(input).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
        insta::assert_snapshot!("adf_to_text_complex", text);
    }

    #[test]
    fn test_markdown_image_is_skipped() {
        let adf = markdown_to_adf("before ![alt](https://example.com/img.png) after").unwrap();
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

    /// An image's alt text must NOT leak into the output. `Tag::Image` pushes a
    /// `NodeKind::Sink`, and the image's alt-text `Event::Text` arrives while the
    /// Sink is on top of the stack — `push_text`'s Sink guard
    /// (`Some(top) if matches!(top.kind, NodeKind::Sink) => return`) drops it.
    ///
    /// F6 (S-522): kills the `matches!(top.kind, NodeKind::Sink) => return` guard
    /// → `false` mutant in `push_text`. With the guard disabled, the alt text
    /// `ALTTEXTMARKER` would be appended as a stray text node. The existing
    /// `test_markdown_image_is_skipped` only asserts the image *URL* is absent,
    /// not the alt text — so it does not exercise this guard. A distinctive,
    /// unambiguous alt-text token pins the drop.
    #[test]
    fn test_markdown_image_alt_text_is_dropped_by_sink_guard() {
        let adf =
            markdown_to_adf("before ![ALTTEXTMARKER](https://example.com/i.png) after").unwrap();
        let serialized = adf.to_string();
        assert!(
            !serialized.contains("ALTTEXTMARKER"),
            "image alt text must be dropped by the push_text Sink guard, \
             not leaked as a stray text node: {adf}"
        );
        // Surrounding text is unaffected (the Sink guard only drops Sink-topped text).
        assert!(
            serialized.contains("before") && serialized.contains("after"),
            "surrounding text must survive: {adf}"
        );
    }

    // --- GFM task lists → ADF taskList/taskItem (issue #471) ----------------
    //
    // BEHAVIOR CHANGE from pre-#471: previously `ENABLE_TASKLISTS` was NOT set,
    // so `- [x]`/`- [ ]` passed through as literal text inside a `bulletList`.
    // The old pinning test was `test_markdown_task_list_syntax_preserved_as_text`.
    // With #471 adding `Options::ENABLE_TASKLISTS`, that literal-text behavior is
    // superseded: GFM task syntax now maps to ADF `taskList`/`taskItem` nodes.
    // The old test is REPLACED (not deleted) with the new tests below — parallel
    // to how #474 replaced `test_markdown_double_tilde_still_strikethrough_not_subscript`
    // when `ENABLE_SUBSCRIPT` changed the single-tilde meaning.
    //
    // BC-7.2.010; S-471; docs/specs/adf-task-list.md

    // --- AC-001 / AC-017 (replacement) : basic forward path -----------------

    #[test]
    fn test_markdown_task_list_emits_task_list_node() {
        // BC-7.2.010 postcondition: `- [ ] unchecked item` → `taskList` containing one
        // `taskItem` with state "TODO". No `bulletList` node in the output.
        // Replaces the pre-#471 test `test_markdown_task_list_syntax_preserved_as_text`
        // which pinned literal-text behavior when ENABLE_TASKLISTS was off.
        let adf = markdown_to_adf("- [ ] unchecked item").unwrap();
        let list = first_block(&adf);
        assert_eq!(list["type"], "taskList", "expected taskList, got: {list}");
        // localId must be a non-empty string
        let local_id = list["attrs"]["localId"].as_str().unwrap_or("");
        assert!(
            !local_id.is_empty(),
            "taskList.attrs.localId must be non-empty: {list}"
        );
        // No bulletList should appear
        assert!(
            !adf.to_string().contains("\"bulletList\""),
            "bulletList must not appear when ENABLE_TASKLISTS is set: {adf}"
        );
        // First (and only) item must be taskItem with state TODO
        let item = &list["content"][0];
        assert_eq!(item["type"], "taskItem", "got: {item}");
        assert_eq!(item["attrs"]["state"], "TODO", "got: {item}");
        // Content text must contain the item text
        assert!(
            adf.to_string().contains("unchecked item"),
            "item text must be preserved: {adf}"
        );
    }

    // --- AC-002 : checked item -----------------------------------------------

    #[test]
    fn test_markdown_task_checked_item_emits_done_state() {
        // BC-7.2.010 EC-1: `- [x] done item` → taskItem with attrs.state == "DONE" (uppercase).
        let adf = markdown_to_adf("- [x] done item").unwrap();
        let list = first_block(&adf);
        assert_eq!(list["type"], "taskList", "got: {list}");
        let item = &list["content"][0];
        assert_eq!(item["type"], "taskItem", "got: {item}");
        assert_eq!(
            item["attrs"]["state"], "DONE",
            "checked item must have state DONE (uppercase): {item}"
        );
    }

    // --- AC-003 : uppercase [X] recognized + reverse renders [x] -------------

    #[test]
    fn test_markdown_task_uppercase_x_emits_done_state() {
        // BC-7.2.010 EC-2: `- [X]` uppercase is recognized as checked.
        // Forward: state must be "DONE" (not "done" or "Done").
        // Reverse (AC-003b): adf_to_text always emits `- [x]` (lowercase) for DONE state.
        // Casing normalization is documented lossiness (EC-10(f)).
        let adf = markdown_to_adf("- [X] uppercase").unwrap();
        let list = first_block(&adf);
        assert_eq!(list["type"], "taskList", "got: {list}");
        let item = &list["content"][0];
        assert_eq!(
            item["attrs"]["state"], "DONE",
            "uppercase [X] must produce state DONE: {item}"
        );
        // Reverse path: must render as `- [x]` (lowercase), never `- [X]`
        let rendered = adf_to_text(&adf).unwrap();
        assert!(
            rendered.contains("- [x] "),
            "DONE state must render as `- [x]` (lowercase), got: {rendered:?}"
        );
        assert!(
            !rendered.contains("- [X] "),
            "`- [X]` must not appear in rendered output (casing normalized): {rendered:?}"
        );
    }

    // --- AC-004 : mixed task + plain list promoted ----------------------------

    #[test]
    fn test_markdown_mixed_task_plain_list_promotes_container() {
        // BC-7.2.010 EC-3: a list containing both task and plain items must have
        // the whole container promoted to `taskList`. Plain items get state "TODO".
        // ADF does not permit mixing `listItem` and `taskItem` in one container.
        let adf = markdown_to_adf("- [ ] checkbox\n- plain item").unwrap();
        let list = first_block(&adf);
        assert_eq!(
            list["type"], "taskList",
            "mixed list must be promoted to taskList: {list}"
        );
        let items = list["content"]
            .as_array()
            .expect("taskList must have content");
        // Both items must be taskItem (no listItem)
        for (i, item) in items.iter().enumerate() {
            assert_eq!(
                item["type"], "taskItem",
                "item[{i}] must be taskItem (not listItem): {item}"
            );
        }
        // Plain item promoted to state TODO
        let plain_item = &items[1];
        assert_eq!(
            plain_item["attrs"]["state"], "TODO",
            "plain item promoted to taskItem must have state TODO: {plain_item}"
        );
        // No listItem nodes anywhere
        assert!(
            !adf.to_string().contains("\"listItem\""),
            "listItem must not appear in mixed task list: {adf}"
        );
    }

    #[test]
    fn test_mixed_task_plain_list_nested_sublist_under_plain_item_preserved() {
        // F-P2-I1: a nested sublist under a plain item in a mixed task+plain list
        // must NOT be dropped. Prior implementation silently skipped non-paragraph
        // blocks in extract_inline_from_list_item_content, losing the sublist.
        //
        // Input: `- [ ] task\n- plain\n  - sub`
        // Expected: `sub` appears in the output (hoisted to the correct level).
        let adf = markdown_to_adf("- [ ] task\n- plain\n  - sub").unwrap();
        let adf_str = adf.to_string();
        assert!(
            adf_str.contains("sub"),
            "nested sublist under plain item in mixed list must be preserved: {adf}"
        );
    }

    #[test]
    fn test_mixed_task_empty_promoted_plain_item_is_pruned() {
        // EC-8 / O-2 consistency: an empty plain item promoted to taskItem in a
        // mixed list must be pruned — identical to how `- [ ]` (an explicit empty
        // task item) is pruned. Before this fix, a bare `- ` plain item in a mixed
        // list would survive as `taskItem { content: [] }` while `- [ ]` was dropped.
        //
        // Input: "- [x] a\n- " — pulldown-cmark emits a Start(Item)→End(Item) for
        // the bare `- ` (no text inside), producing an empty listItem that the
        // promotion arm turns into an empty taskItem. EC-8 requires it be dropped.
        let adf = markdown_to_adf("- [x] a\n- ").unwrap();
        let adf_str = adf.to_string();
        // The result must contain exactly one taskItem (the `[x] a` item).
        // The empty promoted plain item must be pruned.
        let task_list = first_block(&adf);
        assert_eq!(task_list["type"], "taskList", "must be a taskList: {adf}");
        let items = task_list["content"]
            .as_array()
            .expect("taskList must have content array");
        assert_eq!(
            items.len(),
            1,
            "empty promoted plain item must be pruned; expected 1 taskItem, got {}: {adf_str}",
            items.len()
        );
        assert_eq!(items[0]["type"], "taskItem", "only item must be taskItem");
        // Confirm no empty taskItem nodes appear anywhere in the output.
        assert!(
            !adf_str.contains(r#""content":[]"#),
            "no empty-content taskItem must survive pruning: {adf_str}"
        );
    }

    #[test]
    fn test_mixed_task_nonempty_promoted_plain_item_survives() {
        // EC-3 / O-2 regression guard: non-empty plain items promoted to taskItem
        // in a mixed list must NOT be pruned by the emptiness check.
        // Input: "- [ ] task\n- plain" — `plain` is non-empty → must survive.
        let adf = markdown_to_adf("- [ ] task\n- plain").unwrap();
        let task_list = first_block(&adf);
        assert_eq!(task_list["type"], "taskList", "must be taskList: {adf}");
        let items = task_list["content"]
            .as_array()
            .expect("taskList must have content array");
        assert_eq!(
            items.len(),
            2,
            "both items must survive (non-empty): got {}: {adf}",
            items.len()
        );
        assert_eq!(items[0]["type"], "taskItem");
        assert_eq!(items[1]["type"], "taskItem");
        // The promoted plain item has text "plain"
        assert!(
            items[1].to_string().contains("plain"),
            "promoted plain item must contain 'plain': {adf}"
        );
    }

    // --- AC-005 : inline marks preserved in task item -------------------------

    #[test]
    fn test_markdown_task_item_inline_marks_preserved() {
        // BC-7.2.010 EC-4: inline marks (strong, em) are preserved inside taskItem.content.
        // Content goes directly in taskItem.content — NOT wrapped in a paragraph.
        // STRENGTHENED (F-P2-C1): asserts the EXACT content array
        //   [text("bold",[strong]), text(" and "), text("em",[em])]
        // with NO hardBreak nodes between runs. The prior weak version only checked
        // `contains("strong")` and `contains("em")` and missed spurious hardBreaks.
        let adf = markdown_to_adf("- [x] **bold** and _em_").unwrap();
        let list = first_block(&adf);
        assert_eq!(list["type"], "taskList", "got: {list}");
        let item = &list["content"][0];
        assert_eq!(item["type"], "taskItem", "got: {item}");
        let content = item["content"]
            .as_array()
            .expect("taskItem must have content");

        // Content should NOT be wrapped in a paragraph node
        for child in content {
            assert_ne!(
                child["type"], "paragraph",
                "taskItem content must NOT have paragraph wrapper, got: {child}"
            );
        }

        // Exact content array: 3 text nodes, NO hardBreak nodes at all.
        assert_eq!(
            content.len(),
            3,
            "expected exactly 3 inline nodes [bold, ' and ', em], got {}: {}",
            content.len(),
            item
        );
        // No hardBreak nodes anywhere in content (F-P2-C1 regression guard).
        for node in content {
            assert_ne!(
                node["type"], "hardBreak",
                "tight task item must NOT inject spurious hardBreak between inline runs: {}",
                item
            );
        }
        // First node: text "bold" with strong mark
        assert_eq!(
            content[0]["text"], "bold",
            "first node text: {}",
            content[0]
        );
        assert_eq!(
            content[0]["marks"][0]["type"], "strong",
            "first node must have strong mark: {}",
            content[0]
        );
        // Second node: text " and " with no marks
        assert_eq!(
            content[1]["text"], " and ",
            "second node text: {}",
            content[1]
        );
        assert!(
            content[1].get("marks").is_none()
                || content[1]["marks"].as_array().map(|a| a.is_empty()) == Some(true),
            "second node must have no marks: {}",
            content[1]
        );
        // Third node: text "em" with em mark
        assert_eq!(content[2]["text"], "em", "third node text: {}", content[2]);
        assert_eq!(
            content[2]["marks"][0]["type"], "em",
            "third node must have em mark: {}",
            content[2]
        );
    }

    #[test]
    fn test_tight_task_item_inline_code_no_hardbreak() {
        // F-P2-C1 regression: `- [x] a \`code\` b` → tight task item with inline code.
        // The three text runs (plain "a ", code "code", plain " b") must appear
        // as consecutive text/code nodes with NO hardBreak injected between them.
        let adf = markdown_to_adf("- [x] a `code` b").unwrap();
        let list = first_block(&adf);
        assert_eq!(list["type"], "taskList", "got: {list}");
        let item = &list["content"][0];
        assert_eq!(item["type"], "taskItem", "got: {item}");
        let content = item["content"]
            .as_array()
            .expect("taskItem must have content");
        // No hardBreak nodes
        for node in content {
            assert_ne!(
                node["type"], "hardBreak",
                "tight task item with inline code must NOT inject spurious hardBreak: {}",
                item
            );
        }
        // Must contain the code text
        let adf_str = adf.to_string();
        assert!(
            adf_str.contains("\"code\"") && adf_str.contains("code"),
            "inline code mark must be preserved: {adf}"
        );
    }

    #[test]
    fn test_tight_task_item_soft_break_becomes_space_no_hardbreak() {
        // F-P2-C1 regression: `- [x] line one\n  continued` → soft break in a tight
        // task item. pulldown-cmark emits Event::SoftBreak which is mapped to a space.
        // The two text runs must be joined (possibly as one merged text node or two
        // adjacent text nodes) with NO hardBreak injected between them.
        let adf = markdown_to_adf("- [x] line one\n  continued").unwrap();
        let list = first_block(&adf);
        assert_eq!(list["type"], "taskList", "got: {list}");
        let item = &list["content"][0];
        assert_eq!(item["type"], "taskItem", "got: {item}");
        let content = item["content"]
            .as_array()
            .expect("taskItem must have content");
        // No hardBreak nodes anywhere (soft break → space, not hardBreak)
        for node in content {
            assert_ne!(
                node["type"], "hardBreak",
                "soft break in tight task item must NOT become hardBreak: {}",
                item
            );
        }
        // The full text "line one continued" must be present (space may be absorbed)
        let adf_str = adf.to_string();
        assert!(
            adf_str.contains("line one") && adf_str.contains("continued"),
            "both text runs must be present: {adf}"
        );
    }

    // --- AC-006 : task list inside listItem normalized -------------------------

    #[test]
    fn test_task_list_in_list_item_normalized_to_nested_bullet_list() {
        // BC-7.2.010 obligation #1 / EC-5: a task list nested inside a regular list item
        // is normalized by `normalize_list_item_content`'s "taskList" arm.
        // Valid ADF shape: listItem > [bulletList > [listItem > paragraph(...)]]
        // The inner nodes must be `listItem` (NOT `taskItem`).
        // The inner `listItem` must NOT carry a `state` attribute.
        // The checkbox state (TODO/DONE) is dropped — documented lossiness EC-10(b).
        //
        // Anchor assertion: top-level task list (no outer bullet) DOES produce taskList.
        // This fails without ENABLE_TASKLISTS and distinguishes the normalization test
        // from "no taskList because the feature is off" vacuousness.
        let adf_anchor = markdown_to_adf("- [ ] top level").unwrap();
        assert_eq!(
            first_block(&adf_anchor)["type"],
            "taskList",
            "top-level task list must produce taskList (ENABLE_TASKLISTS required): {adf_anchor}"
        );
        let adf = markdown_to_adf("- outer\n  - [ ] inner task").unwrap();
        let outer_list = first_block(&adf);
        // Outer list stays as bulletList (not taskList — the outer item has no checkbox)
        assert_eq!(
            outer_list["type"], "bulletList",
            "outer list must be bulletList: {outer_list}"
        );
        let outer_item = &outer_list["content"][0];
        assert_eq!(outer_item["type"], "listItem", "got: {outer_item}");
        // Find the nested bulletList inside the outer listItem
        let inner_list = outer_item["content"]
            .as_array()
            .expect("outer listItem must have content")
            .iter()
            .find(|n| n["type"] == "bulletList")
            .cloned()
            .expect("outer listItem must contain a bulletList (normalized from taskList)");
        // Inner list items must be listItem (NOT taskItem)
        let inner_items = inner_list["content"]
            .as_array()
            .expect("inner list must have content");
        for (i, inner_item) in inner_items.iter().enumerate() {
            assert_eq!(
                inner_item["type"], "listItem",
                "inner item[{i}] must be listItem (not taskItem): {inner_item}"
            );
            // Must NOT carry a state attribute
            assert!(
                inner_item
                    .get("attrs")
                    .and_then(|a| a.get("state"))
                    .is_none(),
                "inner listItem must not have state attr: {inner_item}"
            );
        }
        // No taskList node should appear anywhere in the output
        assert!(
            !adf.to_string().contains("\"taskList\""),
            "taskList must not appear inside a listItem: {adf}"
        );
        // No taskItem node anywhere
        assert!(
            !adf.to_string().contains("\"taskItem\""),
            "taskItem must not appear inside a listItem: {adf}"
        );
    }

    // --- AC-007 : task list inside blockquote normalized to paragraphs --------

    #[test]
    fn test_task_list_in_blockquote_normalized_to_paragraphs() {
        // BC-7.2.010 obligation #2 / EC-6: unconditional normalization.
        // pulldown-cmark 0.13.3 DOES emit blockquote > taskList for `> - [ ] item`
        // (confirmed by direct source read of firstpass.rs — container-agnostic scan).
        // ADF blockquote.content forbids taskList → normalize to paragraphs.
        //
        // Anchor assertion: top-level task list DOES produce taskList node.
        // This fails without ENABLE_TASKLISTS and guards against vacuous pass
        // ("no taskList in blockquote because the feature is off").
        let adf_anchor = markdown_to_adf("- [ ] top level").unwrap();
        assert_eq!(
            first_block(&adf_anchor)["type"],
            "taskList",
            "top-level task list must produce taskList (ENABLE_TASKLISTS required): {adf_anchor}"
        );
        let adf = markdown_to_adf("> - [ ] item").unwrap();
        let block = first_block(&adf);
        assert_eq!(
            block["type"], "blockquote",
            "blockquote must be preserved: {block}"
        );
        // All children of blockquote must be paragraphs (not taskList)
        let children = block["content"]
            .as_array()
            .expect("blockquote must have content");
        for (i, child) in children.iter().enumerate() {
            assert_ne!(
                child["type"], "taskList",
                "taskList must NOT appear inside blockquote (child[{i}]): {adf}"
            );
            assert_ne!(
                child["type"], "taskItem",
                "taskItem must NOT appear inside blockquote (child[{i}]): {adf}"
            );
        }
        // Item text must still be preserved
        assert!(
            adf.to_string().contains("item"),
            "item text must be preserved in blockquote normalization: {adf}"
        );
    }

    // --- AC-008 : task list inside panel passes through -----------------------

    #[test]
    fn test_task_list_in_panel_passes_through() {
        // BC-7.2.010 obligation #3 / EC-7.
        // LOCKED expected shape: panel(info) > [taskList > [taskItem(state:TODO, content:[text("item")])]]
        //
        // This test also pins the REQUIRED one-line implementation fix (AC-008):
        // the Panel arm's wrap_inlines_as_blocks allowlist (~lines 451-459) must include
        // "taskList". Without it, a surviving taskList is misclassified as inline and
        // wrapped into panel > paragraph > taskList — INVALID ADF (Jira 400).
        // Source: .factory/research/issue-471-panel-tasklist-shape.md §D.
        let adf = markdown_to_adf("> [!NOTE]\n> - [ ] item").unwrap();
        // Must be a panel (from the GFM alert, per #483)
        let panel = first_block(&adf);
        assert_eq!(panel["type"], "panel", "got: {panel}");
        assert_eq!(
            panel["attrs"]["panelType"], "info",
            "[!NOTE] must map to panelType info: {panel}"
        );
        // Panel content must contain a taskList (not paragraph > taskList)
        let panel_children = panel["content"]
            .as_array()
            .expect("panel must have content");
        // The FIRST child of the panel must be a taskList (not a paragraph)
        let first_child = &panel_children[0];
        assert_eq!(
            first_child["type"], "taskList",
            "panel's first child must be taskList, got: {}. \
             If this is 'paragraph', the wrap_inlines_as_blocks allowlist is missing \"taskList\".",
            first_child["type"]
        );
        // taskList must contain a taskItem with state TODO and text "item"
        let task_item = &first_child["content"][0];
        assert_eq!(task_item["type"], "taskItem", "got: {task_item}");
        assert_eq!(
            task_item["attrs"]["state"], "TODO",
            "task item in panel must have state TODO: {task_item}"
        );
        assert!(
            adf.to_string().contains("\"item\""),
            "item text must be preserved in panel task list: {adf}"
        );
    }

    // --- AC-009 : empty task item / task list / hardBreak-only pruned ---------

    #[test]
    fn test_empty_task_item_pruned() {
        // BC-7.2.010 EC-8: `- [ ]` with no text → taskItem has empty content → pruned.
        // The test requires ENABLE_TASKLISTS to be set: first verify that a non-empty
        // task item DOES produce a taskList node (this assertion fails without the feature),
        // then verify the empty item is pruned.
        let adf_nonempty = markdown_to_adf("- [ ] has text").unwrap();
        assert_eq!(
            first_block(&adf_nonempty)["type"],
            "taskList",
            "non-empty task item must produce taskList (requires ENABLE_TASKLISTS): {adf_nonempty}"
        );
        // Now the actual pruning assertion:
        let adf = markdown_to_adf("- [ ]").unwrap();
        let adf_str = adf.to_string();
        assert!(
            !adf_str.contains("\"taskItem\""),
            "empty taskItem must be pruned: {adf}"
        );
    }

    #[test]
    fn test_empty_task_list_pruned() {
        // BC-7.2.010 EC-9: all taskItems pruned → empty taskList → also pruned.
        // Two empty items — both pruned → the enclosing taskList must also be absent.
        // Anchor: a non-empty task list DOES produce a taskList node.
        let adf_nonempty = markdown_to_adf("- [ ] has text").unwrap();
        assert_eq!(
            first_block(&adf_nonempty)["type"],
            "taskList",
            "non-empty task item must produce taskList (requires ENABLE_TASKLISTS): {adf_nonempty}"
        );
        // Empty items: both pruned → taskList also pruned
        let adf = markdown_to_adf("- [ ]\n- [ ]").unwrap();
        let adf_str = adf.to_string();
        assert!(
            !adf_str.contains("\"taskList\""),
            "empty taskList (all items pruned) must be pruned: {adf}"
        );
        assert!(
            !adf_str.contains("\"taskItem\""),
            "empty taskItem must be pruned: {adf}"
        );
    }

    #[test]
    fn test_hardbreak_only_task_item_pruned() {
        // BC-7.2.010 deliberate product choice (EC-008b): a taskItem containing ONLY a
        // hardBreak node is treated as structurally empty and pruned.
        // `- [ ] \\\n` — GFM backslash hard-break produces taskItem.content:[hardBreak]
        // The taskList must also be absent if no items survive.
        //
        // Implementation note (AC-009): adding "taskItem" to REQUIRES_CONTENT is necessary
        // but INSUFFICIENT — a hardBreak-only item has a non-empty content array and will
        // NOT be pruned by the structural membership check alone. A second
        // "structurally-empty inline content" branch (scoped to taskItem only) is required:
        // treat as empty when ALL nodes are hardBreak or whitespace-only text.
        //
        // Anchor: a non-empty task item DOES produce a taskList node (requires ENABLE_TASKLISTS).
        let adf_anchor = markdown_to_adf("- [ ] has text").unwrap();
        assert_eq!(
            first_block(&adf_anchor)["type"],
            "taskList",
            "non-empty task item must produce taskList (requires ENABLE_TASKLISTS): {adf_anchor}"
        );
        let adf = markdown_to_adf("- [ ] \\\n").unwrap();
        let adf_str = adf.to_string();
        assert!(
            !adf_str.contains("\"taskItem\""),
            "hardBreak-only taskItem must be pruned (deliberate product choice): {adf}"
        );
        assert!(
            !adf_str.contains("\"taskList\""),
            "taskList must also be pruned when all items are pruned: {adf}"
        );
    }

    #[test]
    fn test_trim_leading_trailing_hardbreaks_unit() {
        // F6 mutation-kill (src/adf.rs:1759 `if first_non_hb > 0`). Direct unit
        // coverage of the LEADING-trim branch, which no prior test exercised
        // with a positive `first_non_hb` — so the surviving `== 0` and `< 0`
        // (`<`) mutants (both of which disable the leading drain) went
        // undetected. These assertions pin the exact contract: a content array
        // beginning with one or more hardBreaks has them removed, interior
        // hardBreaks are preserved, and trailing hardBreaks are removed.
        let hb = || json!({ "type": "hardBreak" });
        let txt = |s: &str| json!({ "type": "text", "text": s });

        // Leading-only: two leading hardBreaks must be drained (kills `== 0`/`<`).
        let out = trim_leading_trailing_hardbreaks(vec![hb(), hb(), txt("a")]);
        assert_eq!(out, vec![txt("a")], "leading hardBreaks must be trimmed");

        // Leading + interior + trailing: only boundary hardBreaks removed; the
        // interior hardBreak between "a" and "b" is preserved.
        let out = trim_leading_trailing_hardbreaks(vec![hb(), txt("a"), hb(), txt("b"), hb()]);
        assert_eq!(
            out,
            vec![txt("a"), hb(), txt("b")],
            "only leading/trailing hardBreaks trimmed; interior preserved"
        );

        // No leading hardBreak: content unchanged at the front (the `> 0` guard
        // means drain is a no-op here — guards the equivalence boundary).
        let out = trim_leading_trailing_hardbreaks(vec![txt("a"), hb()]);
        assert_eq!(out, vec![txt("a")], "trailing-only hardBreak trimmed");

        // All hardBreaks: drains to empty.
        let out = trim_leading_trailing_hardbreaks(vec![hb(), hb()]);
        assert!(out.is_empty(), "all-hardBreak content drains to empty");
    }

    // --- AC-010 : round-trip stability ----------------------------------------

    #[test]
    fn test_task_list_roundtrip_adf_to_text() {
        // BC-7.2.010 EC-10: round-trip stability.
        // adf_to_text(markdown_to_adf(...)) must produce the exact string
        // `"- [ ] pending\n- [x] done\n"` (taskItem renderer appends `\n`
        // per each item; no trailing blank line). Re-parsing must produce
        // semantically equivalent ADF.
        // localId values are NOT asserted across the text round-trip (they are
        // re-derived from the counter; identical input yields identical IDs).
        //
        // CR-004: strengthened from presence-only `contains` to assert_eq on
        // the trimmed full output, so extraneous-content regressions are caught.
        let input = "- [ ] pending\n- [x] done";
        let adf = markdown_to_adf(input).unwrap();
        let rendered = adf_to_text(&adf).unwrap();
        assert_eq!(
            rendered.trim(),
            "- [ ] pending\n- [x] done",
            "adf_to_text must render exact task list output, got: {rendered:?}"
        );
        // Re-parse must produce taskList (not bulletList)
        let adf2 = markdown_to_adf(&rendered).unwrap();
        let list2 = first_block(&adf2);
        assert_eq!(
            list2["type"], "taskList",
            "re-parsed output must still be taskList: {list2}"
        );
        let items2 = list2["content"].as_array().expect("items");
        assert_eq!(
            items2.len(),
            2,
            "re-parsed taskList must have 2 items: {list2}"
        );
        assert_eq!(
            items2[0]["attrs"]["state"], "TODO",
            "re-parsed pending item must be TODO: {:?}",
            items2[0]
        );
        assert_eq!(
            items2[1]["attrs"]["state"], "DONE",
            "re-parsed done item must be DONE: {:?}",
            items2[1]
        );
    }

    // --- AC-011 : adf_to_text tolerates lowercase state from external ADF -----

    #[test]
    fn test_adf_to_text_external_lowercase_state() {
        // BC-7.2.010 EC-12: an externally-authored ADF taskItem with state "done"
        // (lowercase) must render as `- [x]`. Comparison is case-insensitive.
        let adf = json!({
            "version": 1,
            "type": "doc",
            "content": [{
                "type": "taskList",
                "attrs": { "localId": "1" },
                "content": [{
                    "type": "taskItem",
                    "attrs": { "localId": "2", "state": "done" },
                    "content": [{ "type": "text", "text": "a task" }]
                }]
            }]
        });
        let rendered = adf_to_text(&adf).unwrap();
        assert!(
            rendered.contains("- [x]"),
            "lowercase state 'done' must render as `- [x]`, got: {rendered:?}"
        );
    }

    // --- AC-012 : nested task list (task-in-task) placement + reverse indent --

    #[test]
    fn test_nested_task_list_preserved() {
        // BC-7.2.010 EC-13: `- [ ] outer\n  - [x] nested` →
        // Forward: nested taskList placed as sibling AFTER parent taskItem in parent
        // taskList.content. NOT inside taskItem.content (inline-only).
        // Reverse: adf_to_text renders with exactly 2-space indentation per nesting level.
        let adf = markdown_to_adf("- [ ] outer\n  - [x] nested").unwrap();
        let outer_list = first_block(&adf);
        assert_eq!(outer_list["type"], "taskList", "got: {outer_list}");
        let outer_content = outer_list["content"].as_array().expect("taskList.content");
        // Must have at least 2 elements: the outer taskItem + the nested taskList
        assert!(
            outer_content.len() >= 2,
            "outer taskList.content must contain the outer taskItem AND the nested taskList as siblings: {outer_content:?}"
        );
        // First element must be a taskItem
        assert_eq!(
            outer_content[0]["type"], "taskItem",
            "first element must be taskItem: {:?}",
            outer_content[0]
        );
        // There must be a nested taskList as a sibling (not inside taskItem.content)
        let has_nested_task_list = outer_content.iter().any(|n| n["type"] == "taskList");
        assert!(
            has_nested_task_list,
            "nested taskList must appear as sibling in parent taskList.content: {outer_content:?}"
        );
        // The outer taskItem's content must NOT contain a taskList
        let empty_vec = vec![];
        let outer_item_content = outer_content[0]["content"].as_array().unwrap_or(&empty_vec);
        for child in outer_item_content {
            assert_ne!(
                child["type"], "taskList",
                "taskList must NOT be inside taskItem.content (inline-only): {child}"
            );
        }
        // Reverse path: 2-space indentation pinned
        let rendered = adf_to_text(&adf).unwrap();
        assert!(
            rendered.contains("\n  - [x] nested") || rendered.contains("  - [x] nested"),
            "nested task item must render with exactly 2-space indent, got: {rendered:?}"
        );
    }

    // --- AC-013 : malformed bracket forms stay literal text -------------------

    #[test]
    fn test_malformed_task_markers_stay_literal_text() {
        // BC-7.2.010 EC-14: only `[ ]`, `[x]`, `[X]` are recognized by pulldown-cmark.
        // `[]`, `[*]`, `[-]`, `[  ]`, `[ x]`, `[X ]` produce NO TaskListMarker event
        // → stay as literal text inside a bulletList.
        //
        // Counter-assertion (requires ENABLE_TASKLISTS to be set): the VALID forms DO
        // produce taskList nodes. This assertion fails without the feature and distinguishes
        // "malformed stays as bulletList because feature is off" from
        // "malformed stays as bulletList because pulldown correctly rejects it".
        let valid_forms = [
            ("- [ ] unchecked", "TODO"),
            ("- [x] checked lowercase", "DONE"),
            ("- [X] checked uppercase", "DONE"),
        ];
        for (md, expected_state) in valid_forms {
            let adf = markdown_to_adf(md).unwrap();
            let list = first_block(&adf);
            assert_eq!(
                list["type"], "taskList",
                "valid form {md:?} must produce taskList (ENABLE_TASKLISTS required): {list}"
            );
            assert_eq!(
                list["content"][0]["attrs"]["state"], expected_state,
                "valid form {md:?} must produce state {expected_state}: {:?}",
                list["content"][0]
            );
        }
        // The actual malformed-form assertions:
        let malformed = [
            "- [] no space",
            "- [*] asterisk",
            "- [-] dash",
            "- [  ] double space",
            "- [ x] space before letter",
            "- [X ] trailing space",
        ];
        for md in malformed {
            let adf = markdown_to_adf(md).unwrap();
            let list = first_block(&adf);
            assert_ne!(
                list["type"], "taskList",
                "malformed marker {md:?} must not produce taskList, got: {list}"
            );
            assert_eq!(
                list["type"], "bulletList",
                "malformed marker {md:?} must produce bulletList: {list}"
            );
            assert!(
                !adf.to_string().contains("\"taskItem\""),
                "malformed marker {md:?} must not produce taskItem: {adf}"
            );
        }
    }

    // --- AC-014 : plain list nested inside task item → hoisted ---------------

    #[test]
    fn test_task_item_with_nested_plain_list_hoists_block_sibling() {
        // BC-7.2.010 obligation #4 / EC-15: a plain bulletList nested inside a task
        // item cannot be placed in taskItem.content (inline-only) or as a sibling in
        // taskList.content (only taskItem/taskList permitted). The builder hoists the
        // nested list to the grandparent block level (doc root in this case).
        // Output at grandparent level: [taskList > [taskItem("outer")], bulletList(...)]
        let adf = markdown_to_adf("- [ ] outer\n  - plain inner").unwrap();
        // Must have at least 2 top-level blocks: taskList + hoisted bulletList
        let doc_content = adf["content"].as_array().expect("doc must have content");
        assert!(
            doc_content.len() >= 2,
            "doc must have at least 2 top-level blocks after hoist (taskList + bulletList): {doc_content:?}"
        );
        // First block must be the taskList
        assert_eq!(
            doc_content[0]["type"], "taskList",
            "first top-level block must be taskList: {:?}",
            doc_content[0]
        );
        // Second block must be the hoisted bulletList
        assert_eq!(
            doc_content[1]["type"], "bulletList",
            "second top-level block must be hoisted bulletList: {:?}",
            doc_content[1]
        );
        // The taskList's taskItem must NOT contain a bulletList
        let task_item = &doc_content[0]["content"][0];
        assert_eq!(task_item["type"], "taskItem", "got: {task_item}");
        let empty_vec2 = vec![];
        let item_content = task_item["content"].as_array().unwrap_or(&empty_vec2);
        for child in item_content {
            assert_ne!(
                child["type"], "bulletList",
                "bulletList must NOT appear inside taskItem.content: {child}"
            );
        }
    }

    // --- AC-015 : multi-paragraph task item flattened to inline ---------------

    #[test]
    fn test_task_item_multi_paragraph_flattened_to_inline() {
        // BC-7.2.010 EC-16: paragraph wrappers stripped; hardBreak separator injected.
        // EC-16 runs INSIDE NodeKind::TaskItem arm of end()'s match kind block
        // (before returning to the prune gate), NOT in a post-finish() pass.
        //
        // Sub-assertion 1: normal two-paragraph case
        // `- [ ] line1\n\n  line2` → taskItem.content: [text("line1"), hardBreak, text("line2")]
        let adf1 = markdown_to_adf("- [ ] line1\n\n  line2").unwrap();
        let list1 = first_block(&adf1);
        assert_eq!(list1["type"], "taskList", "got: {list1}");
        let item1 = &list1["content"][0];
        assert_eq!(item1["type"], "taskItem", "got: {item1}");
        // No paragraph wrapper inside taskItem
        let content1 = item1["content"]
            .as_array()
            .expect("taskItem must have content");
        for child in content1 {
            assert_ne!(
                child["type"], "paragraph",
                "taskItem must NOT contain paragraph wrapper: {child}"
            );
        }
        // Must contain a hardBreak separator
        let has_hardbreak = content1.iter().any(|n| n["type"] == "hardBreak");
        assert!(
            has_hardbreak,
            "two-paragraph task item must have hardBreak separator: {content1:?}"
        );
        // Must contain both text nodes
        let text_content: String = content1
            .iter()
            .filter_map(|n| n["text"].as_str())
            .collect::<Vec<_>>()
            .join("");
        assert!(
            text_content.contains("line1") && text_content.contains("line2"),
            "both paragraph texts must be present: {text_content:?}"
        );

        // Sub-assertion 2: trailing-empty-paragraph trim
        // `- [ ] x\n\n  ` → taskItem.content: [text("x")] — NO trailing hardBreak
        let adf2 = markdown_to_adf("- [ ] x\n\n  ").unwrap();
        let list2 = first_block(&adf2);
        assert_eq!(list2["type"], "taskList", "got: {list2}");
        let item2 = &list2["content"][0];
        assert_eq!(item2["type"], "taskItem", "got: {item2}");
        let content2 = item2["content"]
            .as_array()
            .expect("taskItem must have content");
        // Must NOT end with a hardBreak (trim rule)
        if let Some(last) = content2.last() {
            assert_ne!(
                last["type"], "hardBreak",
                "taskItem must NOT end with hardBreak (trim rule): {content2:?}"
            );
        }
        // Must contain the text "x"
        assert!(
            content2.iter().any(|n| n["text"] == "x"),
            "text 'x' must be present: {content2:?}"
        );

        // Sub-assertion 3: both-empty → flatten produces [] → prune fires → taskItem ABSENT
        // EC-16-before-EC-8 ordering: flatten runs first (inside NodeKind::TaskItem arm),
        // producing empty content [], then prune gate fires.
        // If prune runs BEFORE flatten (bug), the unflattened [paragraph(""), paragraph("")]
        // has non-empty content → NOT pruned → stray taskItem PRESENT → this assertion fails.
        let adf3 = markdown_to_adf("- [ ]\n\n  ").unwrap();
        let adf3_str = adf3.to_string();
        assert!(
            !adf3_str.contains("\"taskItem\""),
            "both-empty task item must be pruned (EC-16 flatten before EC-8 prune): {adf3}"
        );
        assert!(
            !adf3_str.contains("\"taskList\""),
            "taskList must also be pruned when only item is pruned: {adf3}"
        );
    }

    // --- AC-016 : native hardBreak inside task item is lossy ------------------

    #[test]
    fn test_task_item_native_hardbreak_inline_is_roundtrip_lossy() {
        // BC-7.2.010 EC-11: a hardBreak is schema-valid in taskItem.content.
        // Round-trip is lossy: adf_to_text renders hardBreak as a newline continuation,
        // but re-parsing a bare newline inside a task item does NOT produce a hardBreak
        // (GFM hardBreak requires two trailing spaces or a backslash).
        //
        // The reverse path MUST emit `- [ ] ` prefix for the TODO taskItem. Without a
        // dedicated taskItem arm in adf_to_text, the renderer falls through to the
        // generic recurse-children path and never emits the `- [ ] ` marker prefix —
        // this assertion will FAIL until the feature is implemented.
        let adf = json!({
            "version": 1,
            "type": "doc",
            "content": [{
                "type": "taskList",
                "attrs": { "localId": "1" },
                "content": [{
                    "type": "taskItem",
                    "attrs": { "localId": "2", "state": "TODO" },
                    "content": [
                        { "type": "text", "text": "before" },
                        { "type": "hardBreak" },
                        { "type": "text", "text": "after" }
                    ]
                }]
            }]
        });
        // Reverse path: renders taskItem with `- [ ] ` prefix (requires taskItem arm in renderer)
        let rendered = adf_to_text(&adf).unwrap();
        assert!(
            rendered.contains("- [ ] "),
            "TODO taskItem must render with `- [ ] ` prefix (requires taskList/taskItem arm in adf_to_text): {rendered:?}"
        );
        assert!(
            rendered.contains("before"),
            "text before hardBreak must appear: {rendered:?}"
        );
        assert!(
            rendered.contains("after"),
            "text after hardBreak must appear: {rendered:?}"
        );
        // Round-trip is lossy: re-parsing the rendered text does NOT produce a hardBreak
        // (bare newline inside `- [ ] item` re-parses as item terminator, not hardBreak)
        let adf2 = markdown_to_adf(&rendered).unwrap();
        let adf2_str = adf2.to_string();
        // The re-parsed ADF is expected to NOT contain a hardBreak inside the task item
        // (this is the documented lossiness — do NOT treat absence as a bug)
        let _ = adf2_str; // Lossiness acknowledged; no assertion on re-parsed hardBreak
    }

    // --- AC-018 : localId DFS preorder assignment ----------------------------

    #[test]
    fn test_task_list_localid_dfs_preorder_assignment() {
        // BC-7.2.010 Required attributes: localIds are assigned in a single
        // post-normalization DFS pre-order walk, 1-based, monotonically increasing,
        // container-before-children. No uuid crate; deterministic.

        // Sub-assertion 1: concrete values for a 2-item list
        // Input: `- [ ] first\n- [x] second`
        // Expected: taskList.localId="1", taskItem[0].localId="2", taskItem[1].localId="3"
        let adf = markdown_to_adf("- [ ] first\n- [x] second").unwrap();
        let task_list = first_block(&adf);
        assert_eq!(task_list["type"], "taskList", "got: {task_list}");
        assert_eq!(
            task_list["attrs"]["localId"], "1",
            "taskList must have localId '1' (container first in DFS preorder): {}",
            task_list["attrs"]["localId"]
        );
        let items = task_list["content"].as_array().expect("taskList.content");
        assert_eq!(
            items[0]["attrs"]["localId"], "2",
            "first taskItem must have localId '2': {}",
            items[0]["attrs"]["localId"]
        );
        assert_eq!(
            items[1]["attrs"]["localId"], "3",
            "second taskItem must have localId '3': {}",
            items[1]["attrs"]["localId"]
        );

        // Sub-assertion 2: dense assignment after pruning (pruned nodes skip counter)
        // Input: `- [ ] keep\n- [ ]\n- [ ] also`
        // Middle item has no text → pruned. Remaining IDs must be dense: "1","2","3"
        let adf2 = markdown_to_adf("- [ ] keep\n- [ ]\n- [ ] also").unwrap();
        let task_list2 = first_block(&adf2);
        assert_eq!(task_list2["type"], "taskList", "got: {task_list2}");
        assert_eq!(
            task_list2["attrs"]["localId"], "1",
            "taskList must have localId '1': {}",
            task_list2["attrs"]["localId"]
        );
        let items2 = task_list2["content"].as_array().expect("taskList2.content");
        assert_eq!(
            items2.len(),
            2,
            "pruned middle item must reduce item count to 2: {items2:?}"
        );
        assert_eq!(
            items2[0]["attrs"]["localId"], "2",
            "first surviving taskItem must have localId '2' (dense, pruned node skips slot): {}",
            items2[0]["attrs"]["localId"]
        );
        assert_eq!(
            items2[1]["attrs"]["localId"], "3",
            "second surviving taskItem must have localId '3': {}",
            items2[1]["attrs"]["localId"]
        );
    }

    // --- F-471-H1: block nodes must not leak into taskItem.content -----------
    // ANY non-inline node that pulldown-cmark emits inside a tight task item
    // (codeBlock, blockquote, heading, table, rule, panel/alert) must be hoisted
    // to a valid sibling level and MUST NOT appear inside taskItem.content.

    /// Helper: recursively assert no node of the given `block_type` appears
    /// anywhere inside any `taskItem.content` in the ADF value tree.
    fn assert_no_block_in_task_item_content(v: &Value, block_type: &str) {
        if v.get("type").and_then(Value::as_str) == Some("taskItem") {
            if let Some(content) = v.get("content").and_then(|c| c.as_array()) {
                for child in content {
                    assert_ne!(
                        child["type"], block_type,
                        "block node type '{}' must NOT appear inside taskItem.content: {}",
                        block_type, child
                    );
                    // Recurse into child in case of deep nesting
                    assert_no_block_in_task_item_content(child, block_type);
                }
            }
        }
        // Recurse into all children/content arrays
        if let Some(arr) = v.get("content").and_then(|c| c.as_array()) {
            for child in arr {
                assert_no_block_in_task_item_content(child, block_type);
            }
        }
    }

    #[test]
    fn test_block_in_tight_task_item_blockquote_hoisted() {
        // F-471-H1: `- [ ] x\n  > quote` — blockquote inside tight task item.
        // The blockquote arrives as a child of the TaskItem in the event stream
        // (tight list, item body is not paragraph-wrapped). It must NOT appear
        // inside taskItem.content (inline-only); it must be hoisted.
        //
        // pulldown-cmark 0.13.3 event stream (verified):
        //   Start(Item) → TaskListMarker(false) → Text("x") →
        //   Start(BlockQuote(None)) → … → End(BlockQuote) → End(Item)
        // So blockquote IS a child of the tight TaskItem node.
        let md = "- [ ] x\n  > quote\n";
        let adf = markdown_to_adf(md).unwrap();
        let serialized = serde_json::to_string(&adf).unwrap();

        // The blockquote must not appear inside any taskItem.content
        assert_no_block_in_task_item_content(&adf, "blockquote");

        // The content must still be valid ADF (taskList present)
        assert!(
            contains_node_type(&adf, "taskList"),
            "result must contain a taskList: {}",
            serialized
        );
    }

    #[test]
    fn test_block_in_loose_task_item_blockquote_hoisted() {
        // F-471-H1: `- [ ] x\n\n  > quote` — blockquote in a loose task item.
        // In a loose list the blockquote appears in ListItem.children (not
        // inside the Paragraph-converted-to-TaskItem), so the loose-ListItem
        // path handles the hoist. The invariant is the same: no blockquote
        // inside taskItem.content.
        let md = "- [ ] x\n\n  > quote\n";
        let adf = markdown_to_adf(md).unwrap();

        assert_no_block_in_task_item_content(&adf, "blockquote");
        assert!(contains_node_type(&adf, "taskList"), "must have taskList");
    }

    #[test]
    fn test_block_in_loose_task_item_code_block_hoisted() {
        // F-471-H1: `- [ ] x\n\n  ```\n  code\n  ```\n` — codeBlock in loose task.
        let md = "- [ ] x\n\n  ```\n  code\n  ```\n";
        let adf = markdown_to_adf(md).unwrap();

        assert_no_block_in_task_item_content(&adf, "codeBlock");
        assert!(contains_node_type(&adf, "taskList"), "must have taskList");
    }

    // --- F-471-M1: empty task body must not prune nested sub-list ----------
    // `- [ ]\n  - [x] nested` (EC-13 with empty outer body) and
    // `- [ ]\n  - plain inner` (EC-15 with empty outer body).
    // Before the fix, is_empty_block_container pruned the empty taskItem
    // BEFORE its hoists were extracted, silently dropping the nested list.

    #[test]
    fn test_empty_task_body_with_nested_task_list_survives() {
        // F-471-M1 / EC-13 + F-PASS3-C1: outer task body empty, nested taskList must
        // survive AND land at a VALID parent (not inside bulletList, which would be
        // `bulletList > taskList` — invalid ADF).
        //
        // Input: `- [ ]\n  - [x] nested`
        // Expected valid shape: doc > taskList{taskItem{"nested"}}
        //   — the empty outer task wrapper is dropped; the nested taskList is
        //     lifted to doc level as a direct child (valid ADF).
        let md = "- [ ]\n  - [x] nested\n";
        let adf = markdown_to_adf(md).unwrap();
        let serialized = serde_json::to_string_pretty(&adf).unwrap();

        // Structural validity: no invalid parent→child relationships.
        assert_valid_adf_structure(&adf);

        // The nested taskList must appear at doc level, not inside a bulletList.
        let doc_children = adf["content"].as_array().expect("doc must have content");
        let first = &doc_children[0];
        assert_eq!(
            first["type"].as_str(),
            Some("taskList"),
            "taskList must be a direct doc child (not wrapped in bulletList): {}",
            serialized
        );

        // The taskItem with "nested" text must be present inside the taskList.
        let task_children = first["content"]
            .as_array()
            .expect("taskList must have content");
        let task_item = &task_children[0];
        assert_eq!(
            task_item["type"].as_str(),
            Some("taskItem"),
            "first child of taskList must be taskItem: {}",
            serialized
        );
        let text = task_item["content"][0]["text"].as_str().unwrap_or("");
        assert_eq!(
            text, "nested",
            "taskItem must contain 'nested' text: {}",
            serialized
        );
    }

    #[test]
    fn test_empty_task_body_with_nested_plain_list_survives() {
        // F-471-M1 / EC-15 + F-PASS3-C1: outer task body empty, nested bulletList must
        // survive AND land at a VALID parent (not inside bulletList, which would be
        // `bulletList > bulletList` — invalid ADF).
        //
        // Input: `- [ ]\n  - plain inner`
        // Expected valid shape: doc > bulletList{listItem{paragraph{"plain inner"}}}
        //   — the empty outer task wrapper + outer list dissolve; the nested bulletList
        //     is lifted to doc level as a direct child (valid ADF).
        let md = "- [ ]\n  - plain inner\n";
        let adf = markdown_to_adf(md).unwrap();
        let serialized = serde_json::to_string_pretty(&adf).unwrap();

        // Structural validity: no invalid parent→child relationships.
        assert_valid_adf_structure(&adf);

        // The nested bulletList must appear at doc level, not inside another bulletList.
        let doc_children = adf["content"].as_array().expect("doc must have content");
        let first = &doc_children[0];
        assert_eq!(
            first["type"].as_str(),
            Some("bulletList"),
            "bulletList must be a direct doc child (not wrapped in another bulletList): {}",
            serialized
        );

        // It must contain a listItem with "plain inner" text.
        let list_children = first["content"]
            .as_array()
            .expect("bulletList must have content");
        let list_item = &list_children[0];
        assert_eq!(
            list_item["type"].as_str(),
            Some("listItem"),
            "bulletList child must be listItem: {}",
            serialized
        );
        let text_val = list_item["content"][0]["content"][0]["text"]
            .as_str()
            .unwrap_or("");
        assert_eq!(
            text_val, "plain inner",
            "listItem must contain 'plain inner' text: {}",
            serialized
        );
    }

    // --- F-471-M2: no underscore-prefixed temp keys in ADF output -----------
    // Guards the invariant that no JSON side-channel field (e.g. _pending_hoists,
    // _post_hoists, or any future temp key) leaks into the serialized ADF.
    // A leak would cause Jira HTTP 400 (additionalProperties: false).

    /// Recursively assert that no JSON object key starts with `_` in the value.
    fn assert_no_underscore_keys(v: &Value, path: &str) {
        match v {
            Value::Object(map) => {
                for key in map.keys() {
                    assert!(
                        !key.starts_with('_'),
                        "temp underscore key '{}' must not appear in ADF output (at {}: {})",
                        key,
                        path,
                        serde_json::to_string(v).unwrap_or_default()
                    );
                }
                for (k, child) in map {
                    assert_no_underscore_keys(child, &format!("{path}.{k}"));
                }
            }
            Value::Array(arr) => {
                for (i, child) in arr.iter().enumerate() {
                    assert_no_underscore_keys(child, &format!("{path}[{i}]"));
                }
            }
            _ => {}
        }
    }

    // --- F-PASS3-I1: structural-validity (parent→child content-model legality) ---
    // A recursive walker that asserts ADF content-model legality for the node
    // types touched by the task-list feature. Runs over the full task-list
    // corpus to permanently guard the parent→child legality class.
    //
    // Rules checked (ADF schema):
    //   • bulletList / orderedList content: every child MUST be `listItem`.
    //   • taskList content:
    //       - first child MUST be `taskItem`.
    //       - subsequent children MUST be `taskItem` or `taskList`.
    //       - NOT bulletList / orderedList / paragraph / etc.
    //   • taskItem content: inline-only — every child MUST be `text` or `hardBreak`.
    //       - NO block nodes (paragraph, bulletList, taskList, codeBlock, …).
    //   • listItem content: must NOT contain `taskList` as a direct child
    //       (taskList is always normalized out during list construction).
    //   • blockquote content: must NOT contain `taskList` as a direct child
    //       (taskList inside a blockquote is flattened to paragraphs).

    /// Recursively validate ADF content-model legality for node types touched
    /// by the task-list feature. Panics with a descriptive message on violation.
    fn assert_valid_adf_structure(v: &Value) {
        assert_valid_adf_node(v, "root");
    }

    fn assert_valid_adf_node(v: &Value, path: &str) {
        let ty = v.get("type").and_then(Value::as_str).unwrap_or("<no-type>");
        if let Some(children) = v.get("content").and_then(Value::as_array) {
            match ty {
                "bulletList" | "orderedList" => {
                    // F-PASS4-I1: minItems:1 — empty list is invalid ADF.
                    assert!(
                        !children.is_empty(),
                        "{ty} must not be empty (minItems:1) \
                         (path: {path}): {}",
                        serde_json::to_string(v).unwrap_or_default()
                    );
                    for (i, child) in children.iter().enumerate() {
                        let child_ty = child
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("<unknown>");
                        assert_eq!(
                            child_ty,
                            "listItem",
                            "{ty} child[{i}] must be listItem but got '{child_ty}' \
                             (path: {path}[{i}]): {}",
                            serde_json::to_string(v).unwrap_or_default()
                        );
                    }
                }
                "taskList" => {
                    // F-PASS4-I1: minItems:1 — empty taskList is invalid ADF.
                    assert!(
                        !children.is_empty(),
                        "taskList must not be empty (minItems:1) \
                         (path: {path}): {}",
                        serde_json::to_string(v).unwrap_or_default()
                    );
                    for (i, child) in children.iter().enumerate() {
                        let child_ty = child
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("<unknown>");
                        if i == 0 {
                            assert_eq!(
                                child_ty,
                                "taskItem",
                                "taskList first child must be taskItem but got '{child_ty}' \
                                 (path: {path}[0]): {}",
                                serde_json::to_string(v).unwrap_or_default()
                            );
                        } else {
                            assert!(
                                child_ty == "taskItem" || child_ty == "taskList",
                                "taskList child[{i}] must be taskItem or taskList but \
                                 got '{child_ty}' (path: {path}[{i}]): {}",
                                serde_json::to_string(v).unwrap_or_default()
                            );
                        }
                    }
                }
                "taskItem" => {
                    // taskItem.content is inline-only: text, hardBreak, and inline
                    // marks (which are represented as text nodes with marks). No
                    // block-level nodes are permitted.
                    const BLOCK_TYPES: &[&str] = &[
                        "paragraph",
                        "bulletList",
                        "orderedList",
                        "taskList",
                        "codeBlock",
                        "blockquote",
                        "table",
                        "panel",
                        "rule",
                        "heading",
                        "mediaSingle",
                        "listItem",
                    ];
                    for (i, child) in children.iter().enumerate() {
                        let child_ty = child
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("<unknown>");
                        assert!(
                            !BLOCK_TYPES.contains(&child_ty),
                            "taskItem must not contain block node '{child_ty}' at \
                             child[{i}] (path: {path}[{i}]): {}",
                            serde_json::to_string(v).unwrap_or_default()
                        );
                    }
                }
                "listItem" => {
                    // F-PASS4-I2: allowlist-based check for listItem children.
                    // ADF listItem.content permits: paragraph, bulletList,
                    // orderedList, codeBlock, mediaSingle.
                    // taskList, heading, blockquote, table, panel, rule, taskItem,
                    // and all other block types are NOT permitted.
                    const ALLOWED: &[&str] = &[
                        "paragraph",
                        "bulletList",
                        "orderedList",
                        "codeBlock",
                        "mediaSingle",
                    ];
                    for (i, child) in children.iter().enumerate() {
                        let child_ty = child
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("<unknown>");
                        assert!(
                            ALLOWED.contains(&child_ty),
                            "listItem child[{i}] must be one of {ALLOWED:?} but got \
                             '{child_ty}' (path: {path}[{i}]): {}",
                            serde_json::to_string(v).unwrap_or_default()
                        );
                    }
                }
                "blockquote" => {
                    // F-PASS4-I2: allowlist-based check for blockquote children.
                    // ADF blockquote.content permits: paragraph, heading,
                    // bulletList, orderedList, codeBlock, rule, mediaSingle,
                    // blockquote. NOT: taskList, table, panel, taskItem, listItem.
                    const ALLOWED: &[&str] = &[
                        "paragraph",
                        "heading",
                        "bulletList",
                        "orderedList",
                        "codeBlock",
                        "rule",
                        "mediaSingle",
                        "blockquote",
                    ];
                    for (i, child) in children.iter().enumerate() {
                        let child_ty = child
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("<unknown>");
                        assert!(
                            ALLOWED.contains(&child_ty),
                            "blockquote child[{i}] must be one of {ALLOWED:?} but got \
                             '{child_ty}' (path: {path}[{i}]): {}",
                            serde_json::to_string(v).unwrap_or_default()
                        );
                    }
                }
                "panel" => {
                    // F-PASS4-I2: allowlist-based check for panel children.
                    // ADF panel.content permits: paragraph, heading, bulletList,
                    // orderedList, taskList, codeBlock, rule, mediaSingle.
                    // NOT: table, panel (nested), blockquote, listItem, taskItem.
                    const ALLOWED: &[&str] = &[
                        "paragraph",
                        "heading",
                        "bulletList",
                        "orderedList",
                        "taskList",
                        "codeBlock",
                        "rule",
                        "mediaSingle",
                    ];
                    for (i, child) in children.iter().enumerate() {
                        let child_ty = child
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("<unknown>");
                        assert!(
                            ALLOWED.contains(&child_ty),
                            "panel child[{i}] must be one of {ALLOWED:?} but got \
                             '{child_ty}' (path: {path}[{i}]): {}",
                            serde_json::to_string(v).unwrap_or_default()
                        );
                    }
                }
                _ => {}
            }
            // Recurse into all children regardless of node type.
            for (i, child) in children.iter().enumerate() {
                assert_valid_adf_node(child, &format!("{path}.{ty}[{i}]"));
            }
        }
    }

    #[test]
    fn test_adf_structural_validity_task_list_corpus() {
        // F-PASS3-I1 + F-PASS4-C1: structural-validity corpus test.
        // Run assert_valid_adf_structure across ALL task-list inputs: basic,
        // nested, mixed, panel, blockquote, multi-paragraph, empty-body cases,
        // the F-PASS3-C1 trigger inputs, and the F-PASS4-C1 order-composition
        // inputs (hoisted blocks interleaved with task items in various orders).
        let inputs: &[(&str, &str)] = &[
            // Basic task list
            ("basic task unchecked", "- [ ] task\n"),
            ("basic task checked", "- [x] done\n"),
            // Multiple items
            ("two tasks", "- [ ] first\n- [x] second\n"),
            // EC-3: mixed task + plain items
            ("mixed task+plain", "- [ ] task\n- plain\n"),
            // EC-13: nested taskList inside task item
            ("nested task list", "- [ ] outer\n  - [x] nested\n"),
            // EC-15: nested plain list inside task item
            ("nested plain in task", "- [ ] outer\n  - plain inner\n"),
            // F-PASS3-C1 trigger inputs (empty task body + nested sub-list)
            ("empty task + nested task (C1)", "- [ ]\n  - [x] nested\n"),
            ("empty task + nested plain (C1)", "- [ ]\n  - plain inner\n"),
            // F-471-M1: empty outer + nested checked task
            ("empty task body + nested checked", "- [ ]\n  - [x] done\n"),
            // Loose task lists (EC-16)
            ("loose task", "- [ ] line1\n\n  line2\n"),
            // Block inside tight task item (F-471-H1)
            ("blockquote inside task", "- [ ] x\n  > quote\n"),
            ("codeblock inside task", "- [ ] x\n\n  ```\n  code\n  ```\n"),
            // Panel containing task list
            ("panel with task", "> [!NOTE]\n> - [ ] in panel\n"),
            // Blockquote with task list (normalized to paragraphs)
            ("blockquote with task", "> - [ ] in blockquote\n"),
            // Plain ordered list — no task markers, must stay orderedList (regression guard)
            ("ordered list plain", "1. first\n2. second\n"),
            // Nested ordered list inside plain list
            ("nested ordered in plain", "- item\n  1. sub\n"),
            // EC-ordered: ordered list with task markers → reclassified to taskList
            (
                "ordered task list unchecked+checked",
                "1. [ ] a\n2. [x] b\n",
            ),
            ("ordered task list mixed", "1. [ ] a\n2. plain\n"),
            ("ordered task list nested", "1. [ ] a\n   1. [ ] b\n"),
            // Deeply nested: task inside task inside task
            ("triple nested task", "- [ ] a\n  - [ ] b\n    - [x] c\n"),
            // Task with URL (bare URL autolinking should not break structure)
            ("task with url", "- [ ] see https://example.com\n"),
            // Multi-paragraph loose task item (EC-16)
            (
                "multi-para loose task",
                "- [ ] para one\n\n  para two\n\n  para three\n",
            ),
            // F-PASS4-C1: order-composition inputs (hoisted blocks interleaved)
            // Empty task first, plain nested, then real task item.
            // Expected: [bulletList(plain), taskList([after])]
            (
                "F-PASS4-C1: empty+plain hoist before task",
                "- [ ]\n  - plain inner\n- [x] after\n",
            ),
            // Real task, then empty with plain nested, then real task.
            // Expected: [taskList([before]), bulletList(plain), taskList([after])]
            (
                "F-PASS4-C1: task hoist task interleaved",
                "- [x] before\n- [ ]\n  - plain\n- [x] after\n",
            ),
            // Empty parent, two nested task items.
            // Expected: [taskList([a, b])]
            (
                "F-PASS4-C1: empty parent two nested tasks",
                "- [ ]\n  - [x] a\n  - [ ] b\n",
            ),
            // Real task, then empty with nested task.
            // Expected: both 'real' and 'nested' present in order.
            (
                "F-PASS4-C1: real task then empty with nested task",
                "- [x] real\n- [ ]\n  - [ ] nested\n",
            ),
            // F6-P1 (proptest-minimized): a plain item carrying a nested
            // task-sublist, followed by a sibling task item. The nested sublist
            // hoists a lone `taskList` ahead of the next task run; without the
            // tuple-lead guard in reclassify_as_task_list this produced an
            // invalid `taskList > taskList` (first child a taskList, not a
            // taskItem). Both the panel-wrapped (original minimized) and bare
            // forms are pinned.
            (
                "F6-P1: nested plain-task then task (bare)",
                "- [ ] o\n  - p\n    - [ ] deep\n  - [ ] sib\n",
            ),
            (
                "F6-P1: nested plain-task then task (in panel)",
                "> [!NOTE]\n> - [ ] x\n>   - x\n>     - [ ] x\n>   - [ ] x\n",
            ),
            (
                "F6-P1: minimal lone-taskList-before-task",
                "- p\n    - [ ] deep\n- [ ] sib\n",
            ),
        ];
        for (label, md) in inputs {
            let adf = markdown_to_adf(md).unwrap();
            // No underscore keys (F-471-M2)
            assert_no_underscore_keys(&adf, "root");
            // Structural validity (F-PASS3-I1)
            // Wrap in a catch to emit label on failure
            // assert_valid_adf_structure panics on violation; the panic message
            // includes the path + node JSON.  We call it directly — the label
            // is captured via `assert_no_empty_list_content` below.
            assert_valid_adf_structure(&adf);
            // Additionally verify no empty content arrays for list nodes
            // (empty bulletList/taskList are invalid ADF)
            assert_no_empty_list_content(&adf, label);
        }
    }

    #[test]
    fn test_task_list_no_tasklist_leading_child_f6_p1() {
        // F6-P1 regression (proptest-minimized). A plain item carrying a nested
        // task-sublist, followed by a sibling task item, hoists a lone
        // `taskList` ahead of the next task run. Before the tuple-lead guard in
        // `reclassify_as_task_list`, this wrapped the lone taskList into a fresh
        // `taskList` — producing the invalid `taskList > taskList` (first child
        // a taskList instead of a taskItem), which Jira rejects with HTTP 400.
        //
        // Pin the precise invariant: every `taskList` node's FIRST child is a
        // `taskItem`, on the exact minimized inputs proptest discovered.
        let inputs = [
            "- [ ] o\n  - p\n    - [ ] deep\n  - [ ] sib\n",
            "> [!NOTE]\n> - [ ] x\n>   - x\n>     - [ ] x\n>   - [ ] x\n",
            "- p\n    - [ ] deep\n- [ ] sib\n",
        ];
        for md in inputs {
            let adf = markdown_to_adf(md).unwrap();
            assert_every_tasklist_leads_with_taskitem(&adf, md);
            // Defense in depth: the full structural validator must also pass.
            assert_valid_adf_structure(&adf);
        }
    }

    /// Assert the ADF taskList tuple-lead rule on every taskList in the tree:
    /// `taskList = (taskItem, (taskItem | taskList)*)` — the FIRST child must be
    /// a `taskItem`, never a `taskList`.
    fn assert_every_tasklist_leads_with_taskitem(v: &Value, md: &str) {
        if v.get("type").and_then(Value::as_str) == Some("taskList") {
            let first_ty = v
                .get("content")
                .and_then(Value::as_array)
                .and_then(|c| c.first())
                .and_then(|n| n.get("type"))
                .and_then(Value::as_str);
            assert_eq!(
                first_ty,
                Some("taskItem"),
                "taskList first child must be taskItem (input {md:?}): {}",
                serde_json::to_string(v).unwrap_or_default()
            );
        }
        if let Some(children) = v.get("content").and_then(Value::as_array) {
            for child in children {
                assert_every_tasklist_leads_with_taskitem(child, md);
            }
        }
    }

    /// Walk the ADF and assert that no bulletList, orderedList, or taskList has
    /// an empty content array (empty list containers are invalid ADF).
    fn assert_no_empty_list_content(v: &Value, label: &str) {
        let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
        if matches!(ty, "bulletList" | "orderedList" | "taskList") {
            let content = v.get("content").and_then(Value::as_array);
            assert!(
                content.map(|c| !c.is_empty()).unwrap_or(false),
                "[{label}] {ty} must not have empty content: {}",
                serde_json::to_string(v).unwrap_or_default()
            );
        }
        if let Some(children) = v.get("content").and_then(Value::as_array) {
            for child in children {
                assert_no_empty_list_content(child, label);
            }
        }
    }

    #[test]
    fn test_no_temp_underscore_keys_in_adf_output() {
        // F-471-M2: leak guard. Run a representative set of task-list inputs that
        // exercise both the EC-13 nested-taskList path and the EC-15 hoist path,
        // plus panel+task and blockquote+task, and assert zero underscore keys.
        let inputs = [
            // EC-13: nested task list sibling
            "- [ ] outer\n  - [x] nested\n",
            // EC-15: nested plain list inside task item
            "- [ ] outer\n  - plain inner\n",
            // F-471-M1: empty outer task body with nested task list
            "- [ ]\n  - [x] nested\n",
            // F-471-M1: empty outer task body with nested plain list
            "- [ ]\n  - plain inner\n",
            // Panel containing task list
            "> [!NOTE]\n> - [ ] in panel\n",
            // Blockquote containing task list (normalized to paragraphs)
            "> - [ ] in blockquote\n",
            // Mixed task+plain list (EC-3)
            "- [ ] task\n- plain\n",
        ];
        for md in &inputs {
            let adf = markdown_to_adf(md).unwrap();
            assert_no_underscore_keys(&adf, "root");
        }
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
        let adf = markdown_to_adf("See note.[^1]\n\n[^1]: The note body.").unwrap();
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
        let adf = markdown_to_adf("Body.[^1]\n\n[^1]: Definition text.").unwrap();
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
        let adf = markdown_to_adf("Body.[^1]\n\n[^1]: Definition text.").unwrap();
        let s = adf.to_string();
        assert!(
            !s.contains("[^1]:"),
            "stray `[^1]:` definition remnant must not survive: {s}"
        );
    }

    #[test]
    fn test_markdown_multiple_footnotes_share_single_divider() {
        let adf = markdown_to_adf("First.[^a] Second.[^b]\n\n[^a]: Alpha.\n[^b]: Beta.").unwrap();
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
        let adf = markdown_to_adf("Dangling.[^x]").unwrap();
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
        let adf = markdown_to_adf("Body.[^1]\n\n> [^1]: quoted note").unwrap();
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
        let adf = markdown_to_adf("Body.[^1]\n\n- [^1]: listed note").unwrap();
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
        let adf = markdown_to_adf("**bold[^1]**\n\n[^1]: note").unwrap();
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
        let adf = markdown_to_adf("Body.[^1]\n\n[^1]: first\n[^1]: second").unwrap();
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
        let adf = markdown_to_adf("Body.[^1]\n\n---\n\n[^1]: note").unwrap();
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
        let adf = markdown_to_adf("[^1]: orphan definition").unwrap();
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
        let adf = markdown_to_adf("Body.[^1]\n\n[^1]:\n    - alpha\n    - beta").unwrap();
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
        // CR-012: also covers taskList + taskItem (the two types added in #471).
        for ty in [
            "blockquote",
            "panel",
            "heading",
            "listItem",
            "bulletList",
            "orderedList",
            "table",
            "tableRow",
            "taskList",
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
        // taskItem has extended emptiness semantics: hardBreak-only / whitespace-only
        // / backslash-only content is also prunable (BC-7.2.010 EC-8 deliberate choice).
        assert!(
            is_empty_block_container(&json!({ "type": "taskItem", "content": [] })),
            "taskItem with empty content must be pruned"
        );
        assert!(
            is_empty_block_container(
                &json!({ "type": "taskItem", "content": [{ "type": "hardBreak" }] })
            ),
            "taskItem with hardBreak-only content must be pruned (extended empty)"
        );
        assert!(
            is_empty_block_container(
                &json!({ "type": "taskItem", "content": [{ "type": "text", "text": "   " }] })
            ),
            "taskItem with whitespace-only text must be pruned (extended empty)"
        );
        assert!(
            is_empty_block_container(
                &json!({ "type": "taskItem", "content": [{ "type": "text", "text": "\\\\" }] })
            ),
            "taskItem with backslash-only text must be pruned (extended empty, failed-escape artifact)"
        );
        // A taskItem with real text content must NOT be pruned.
        assert!(
            !is_empty_block_container(
                &json!({ "type": "taskItem", "content": [{ "type": "text", "text": "x" }] })
            ),
            "taskItem with real text content must NOT be pruned"
        );
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
        let adf = markdown_to_adf("#\n\nbody text").unwrap();
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
        let adf = markdown_to_adf("a ^sup^ b").unwrap();
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
        let adf = markdown_to_adf("a ~sub~ b").unwrap();
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
        let adf = markdown_to_adf("mc^2^").unwrap();
        let t = adf["content"][0]["content"][0].clone();
        assert_eq!(t["text"], "mc^2^", "intraword caret stays literal: {t}");
        assert!(t["marks"].is_null(), "no subsup mark applied: {t}");
    }

    #[test]
    fn test_markdown_double_tilde_still_strikethrough_not_subscript() {
        // Enabling ENABLE_SUBSCRIPT must not steal `~~x~~` from strikethrough.
        let adf = markdown_to_adf("~~struck~~").unwrap();
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
        assert_eq!(adf_to_text(&sup).unwrap().trim(), "^x^");
        let sub = json!({
            "type": "doc",
            "content": [{ "type": "paragraph", "content": [
                { "type": "text", "text": "y", "marks": [{ "type": "subsup", "attrs": { "type": "sub" } }] }
            ]}]
        });
        assert_eq!(adf_to_text(&sub).unwrap().trim(), "~y~");
    }

    #[test]
    fn test_subsup_markdown_to_text_roundtrip() {
        let text = adf_to_text(&markdown_to_adf("a ^sup^ and ~sub~ b").unwrap()).unwrap();
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
        let adf = markdown_to_adf("**^x^**").unwrap();
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
        let reparsed = markdown_to_adf(&adf_to_text(&adf).unwrap()).unwrap();
        let rt = marks_of(&reparsed);
        assert!(
            rt.contains(&"strong".to_string()) && rt.contains(&"subsup".to_string()),
            "round-trip preserves both marks: {reparsed}"
        );
    }

    #[test]
    fn test_markdown_strike_sub_sup_coexist() {
        let adf = markdown_to_adf("~~s~~ ~b~ ^p^").unwrap();
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
        let adf = markdown_to_adf("a ^b ~c~ d^ e").unwrap();
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
        let adf = markdown_to_adf("a ^b ~c~ d^ e").unwrap();
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
        let adf_sup = markdown_to_adf("a ^sup^ b").unwrap();
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
        let adf_sub = markdown_to_adf("a ~sub~ b").unwrap();
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
        let adf = markdown_to_adf("## Foo {bar}").unwrap();
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
            let adf = markdown_to_adf(src).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
        assert!(text.contains("| just text |"), "cell not flat: {text:?}");
    }

    #[test]
    fn test_markdown_table_cell_with_inline_formatting() {
        // Verify marks (Task 2) compose correctly with table cells (Task 3).
        // Structure: doc > table > tableRow > tableHeader > paragraph > text.
        let adf =
            markdown_to_adf("| **bold** | [link](https://x) |\n| - | - |\n| a | b |").unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        assert_eq!(adf_to_text(&adf).unwrap(), "**bold**");
    }

    #[test]
    fn test_render_em_mark() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "em", "marks": [{"type": "em"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf).unwrap(), "*em*");
    }

    #[test]
    fn test_render_strike_mark() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "gone", "marks": [{"type": "strike"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf).unwrap(), "~~gone~~");
    }

    #[test]
    fn test_render_code_mark() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "x", "marks": [{"type": "code"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf).unwrap(), "`x`");
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
        assert_eq!(adf_to_text(&adf).unwrap(), "[jr](https://example.com/jr)");
    }

    #[test]
    fn test_render_link_missing_href_defaults_empty() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "jr", "marks": [{"type": "link"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf).unwrap(), "[jr]()");
    }

    #[test]
    fn test_render_multiple_marks_deterministic_order() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "foo", "marks": [{"type": "strong"}, {"type": "em"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf).unwrap(), "***foo***");
    }

    #[test]
    fn test_render_unknown_mark_drops_syntax() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "plain", "marks": [{"type": "underline"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf).unwrap(), "plain");
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let adf_original = markdown_to_adf(input).unwrap();
        let text = adf_to_text(&adf_original).unwrap();
        let adf_reparsed = markdown_to_adf(&text).unwrap();

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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        assert_eq!(adf_to_text(&adf).unwrap(), "");
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
        assert_eq!(adf_to_text(&adf).unwrap(), "");
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
        assert_eq!(adf_to_text(&adf).unwrap(), "a\n\nb");
    }

    #[test]
    fn test_render_marks_code_and_strong() {
        // Externally-produced or legacy ADF that we must render tolerantly.
        // This test covers the reverse-order case: even when the array is
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
        assert_eq!(adf_to_text(&adf).unwrap(), "**`x`**");
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
        assert_eq!(adf_to_text(&adf).unwrap(), "*~~x~~*");
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
        assert_eq!(
            adf_to_text(&adf).unwrap(),
            "**[x](https://example.com/jr)**"
        );
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
        assert_eq!(adf_to_text(&adf).unwrap(), "a");
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
        let text = adf_to_text(&adf).unwrap();
        // The cell content must stay on a single pipe row — no embedded newline.
        assert!(text.contains("| line one line two |"), "got: {text:?}");
    }

    #[test]
    fn test_render_strong_with_code_applies_code_innermost() {
        // Externally-produced or legacy ADF that we must render tolerantly.
        // Output must be **`x`** not `**x**`.
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "x", "marks": [{"type": "strong"}, {"type": "code"}]}
            ]}]
        });
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let adf = markdown_to_adf("> [!NOTE]\n> useful info").unwrap();
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
        let adf = markdown_to_adf("> [!TIP]\n> a tip").unwrap();
        assert_panel(&adf, "success");
    }

    #[test]
    fn test_markdown_alert_important_maps_to_panel_note() {
        let adf = markdown_to_adf("> [!IMPORTANT]\n> key point").unwrap();
        assert_panel(&adf, "note");
    }

    #[test]
    fn test_markdown_alert_warning_maps_to_panel_warning() {
        let adf = markdown_to_adf("> [!WARNING]\n> careful").unwrap();
        assert_panel(&adf, "warning");
    }

    #[test]
    fn test_markdown_alert_caution_maps_to_panel_error() {
        let adf = markdown_to_adf("> [!CAUTION]\n> danger").unwrap();
        assert_panel(&adf, "error");
    }

    #[test]
    fn test_markdown_alert_marker_with_trailing_text_stays_literal_blockquote() {
        // pulldown-cmark 0.13 requires the alert marker to be the SOLE content of
        // the first line. Trailing text on the marker line (`> [!NOTE] extra`)
        // disqualifies it -> stays a plain blockquote with the marker as literal
        // text. (Note: pulldown is otherwise lenient — a missing space `>[!NOTE]`
        // and any-case `[!note]`/`[!Note]` ARE still recognized as alerts.)
        let adf = markdown_to_adf("> [!NOTE] extra\n> text").unwrap();
        let block = first_block(&adf);
        assert_eq!(block["type"], "blockquote", "got: {block}");
        assert!(
            adf.to_string().contains("[!NOTE]"),
            "marker stays literal: {adf}"
        );
    }

    #[test]
    fn test_markdown_unknown_alert_kind_stays_literal_blockquote() {
        let adf = markdown_to_adf("> [!FOO]\n> text").unwrap();
        let block = first_block(&adf);
        assert_eq!(block["type"], "blockquote", "got: {block}");
        assert!(
            adf.to_string().contains("[!FOO]"),
            "unknown marker stays literal: {adf}"
        );
    }

    #[test]
    fn test_markdown_plain_blockquote_unchanged() {
        let adf = markdown_to_adf("> just a quote").unwrap();
        let block = first_block(&adf);
        assert_eq!(block["type"], "blockquote", "got: {block}");
    }

    #[test]
    fn test_markdown_nested_alert_unwraps_inner_panel() {
        // `panel > panel` is invalid ADF; the inner alert is unwrapped, its blocks
        // spliced into the outer panel. No panel may contain a nested panel.
        let adf = markdown_to_adf("> [!NOTE]\n> outer\n> > [!TIP]\n> > inner").unwrap();
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
        let adf = markdown_to_adf(md).unwrap();
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
        let adf = markdown_to_adf(md).unwrap();
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
        let adf = markdown_to_adf("> [!NOTE]\n> # Title").unwrap();
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
        let adf = markdown_to_adf("> [!NOTE]").unwrap();
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
        let adf = markdown_to_adf(md).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
        let text = adf_to_text(&adf).unwrap();
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
            let adf = markdown_to_adf(&md).unwrap();
            let text = adf_to_text(&adf).unwrap();
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
        let adf = markdown_to_adf(">[!NOTE]\n>body").unwrap();
        assert_panel(&adf, "info");
    }

    #[test]
    fn test_markdown_alert_marker_case_insensitive_still_panel() {
        // pulldown-cmark recognizes the marker case-insensitively (`[!note]`,
        // `[!Note]`). Pin it so a dependency bump can't silently regress it.
        for md in ["> [!note]\n> b", "> [!Note]\n> b", "> [!WaRnInG]\n> b"] {
            let adf = markdown_to_adf(md).unwrap();
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
        let adf = markdown_to_adf("1. a\n   1. b").unwrap();

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
    ///    `Tag::HtmlBlock` (Start + End) wrapping `Event::Html` line events. ADF
    ///    has no raw-HTML node, but silently discarding the source is data loss,
    ///    so we preserve the verbatim block as literal text inside a paragraph
    ///    (issue #489 — preserve vs drop). The three asymmetries from inline HTML
    ///    (own paragraph wrapper, trailing-`\r`/`\n` trim, no active marks) are
    ///    inherent to the #489 preserve-not-drop handler. Issue #492 added
    ///    Algorithm B: interior newlines are represented as `hardBreak` nodes
    ///    instead of raw `\n` in text (Jira rejects raw `\n`). Detail:
    ///    `docs/specs/adf-block-html.md` §"Differences from inline HTML".
    #[test]
    fn test_convert_block_html_is_preserved_as_literal_text() {
        // `<div>x</div>` on its own line: pulldown-cmark emits
        //   Start(HtmlBlock) → Html("<div>x</div>\n") → End(HtmlBlock).
        // The HtmlBlock end handler concatenates the inner Html lines, trims the
        // single trailing block newline, and emits a paragraph of literal text.
        let adf = markdown_to_adf("<div>x</div>").unwrap();
        let content = adf["content"].as_array().unwrap();
        assert_eq!(
            content.len(),
            1,
            "block HTML must be preserved as one paragraph, not dropped: {adf}"
        );
        assert_eq!(
            content[0]["type"], "paragraph",
            "block HTML wraps in a paragraph: {adf}"
        );
        assert_eq!(
            content[0]["content"][0]["type"], "text",
            "block HTML body is a literal text node: {adf}"
        );
        assert_eq!(
            content[0]["content"][0]["text"], "<div>x</div>",
            "block HTML preserved verbatim (trailing block newline trimmed): {adf}"
        );
        // No styling marks on raw-HTML literal text.
        assert!(
            content[0]["content"][0].get("marks").is_none(),
            "literal HTML text must carry no marks: {adf}"
        );
    }

    /// BC-7.2.011 AC-004 — REPLACED from issue #489 body (which asserted the
    /// buggy raw-`\n`-in-text behavior). The new body asserts Algorithm B's
    /// hardBreak-segmented `content` array per Algorithm B's general multiline hardBreak-segmentation (AC-004).
    ///
    /// Input `<div>\n  <span>x</span>\n</div>` has 3 segments after trim+split:
    /// `["<div>", "  <span>x</span>", "</div>"]`.
    /// Expected content: `[text("<div>"), hardBreak, text("  <span>x</span>"), hardBreak, text("</div>")]`.
    /// No raw `\n` may appear in any text-node string (file-wide invariant).
    ///
    /// RED GATE: fails against pre-#492 handler (which emits one text node
    /// with raw `\n` characters in the `"text"` field).
    #[test]
    fn test_convert_multiline_block_html_preserves_interior_newlines() {
        // BC-7.2.011 Algorithm B (general multiline hardBreak-segmentation); AC-004.
        let adf = markdown_to_adf("<div>\n  <span>x</span>\n</div>").unwrap();
        let content = adf["content"].as_array().unwrap();
        assert_eq!(
            content.len(),
            1,
            "multi-line block HTML must be one paragraph: {adf}"
        );
        let para_content = content[0]["content"].as_array().unwrap();
        // Algorithm B: 3 segments → 2 boundaries → [text, hb, text, hb, text].
        assert_eq!(
            para_content.len(),
            5,
            "3 segments must produce 5 content nodes (text, hardBreak, text, hardBreak, text): {para_content:?}"
        );
        assert_eq!(
            para_content[0]["type"], "text",
            "first node must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[0]["text"], "<div>",
            "first text node must be '<div>': {para_content:?}"
        );
        assert_eq!(
            para_content[1]["type"], "hardBreak",
            "second node must be hardBreak: {para_content:?}"
        );
        assert_eq!(
            para_content[2]["type"], "text",
            "third node must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[2]["text"], "  <span>x</span>",
            "third text node must be '  <span>x</span>': {para_content:?}"
        );
        assert_eq!(
            para_content[3]["type"], "hardBreak",
            "fourth node must be hardBreak: {para_content:?}"
        );
        assert_eq!(
            para_content[4]["type"], "text",
            "fifth node must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[4]["text"], "</div>",
            "fifth text node must be '</div>': {para_content:?}"
        );
        // File-wide invariant: no raw \n in any text node.
        for node in para_content {
            if node["type"] == "text" {
                let text_val = node["text"].as_str().unwrap_or("");
                assert!(
                    !text_val.contains('\n'),
                    "text node must not contain raw \\n (file-wide invariant): {node:?}"
                );
            }
        }
    }

    /// Block HTML round-trips through `adf_to_text` without loss or duplication.
    /// Issue #489 acceptance: "round-trip / adf_to_text behavior considered".
    /// Single-line path — unchanged from #489. Must continue to PASS.
    #[test]
    fn test_block_html_round_trips_through_adf_to_text() {
        let adf = markdown_to_adf("<div>x</div>").unwrap();
        let text = adf_to_text(&adf).unwrap();
        assert_eq!(
            text, "<div>x</div>",
            "block HTML must survive the ADF→text round trip verbatim: {text:?}"
        );
    }

    // --- BC-7.2.011 new tests (issue #492 RED GATE suite) -----------------------

    /// BC-7.2.011 EC-5 extended (AC-005 test 1) — multi-line block HTML whose
    /// final line is non-whitespace round-trips byte-identically through
    /// `adf_to_text` (line-structure lossless; LF-only input; no trailing
    /// whitespace line; no blank-line-terminated HTML block split → byte-identical).
    ///
    /// Input: `"<div>\n  <span>a</span>\n</div>"` — LF-only, three lines, no
    /// interior blank line (avoids pulldown-cmark's blank-line HTML-block
    /// termination which splits at blank lines per CommonMark §4.6 type 6 rule).
    /// Expected round-trip output: `"<div>\n  <span>a</span>\n</div>"`.
    ///
    /// This test has TWO assertions:
    ///   (a) Forward ADF structure: 5-node hardBreak-segmented content array
    ///       (RED GATE — fails against pre-#492 handler which emits one text
    ///       node with a raw `\n` field, producing exactly 1 content node).
    ///   (b) Round-trip: rendered string equals original input
    ///       (regression guard — note: round-trip is also byte-identical
    ///       against the buggy pre-#492 handler, so this assertion alone is
    ///       NOT a red gate; assertion (a) is the gating check).
    #[test]
    fn test_multiline_block_html_round_trips_through_adf_to_text() {
        // BC-7.2.011 EC-5 extended. Input: LF-only, no interior blank line,
        // ends on "</div>" (non-whitespace) → byte-identical round-trip.
        // No blank line means pulldown-cmark delivers this as one HtmlBlock.
        let input = "<div>\n  <span>a</span>\n</div>";
        let adf = markdown_to_adf(input).unwrap();

        // (a) Forward structure: Algorithm B produces [text, hb, text, hb, text].
        // This is the RED GATE — pre-#492 handler emits a single text node with
        // raw `\n`, so `para_content.len()` would be 1, not 5.
        let para_content = adf["content"][0]["content"].as_array().unwrap();
        assert_eq!(
            para_content.len(),
            5,
            "3 segments must produce 5 content nodes [text, hb, text, hb, text]: {para_content:?}"
        );
        assert_eq!(
            para_content[1]["type"], "hardBreak",
            "node[1] must be hardBreak (not raw \\n in text): {para_content:?}"
        );
        assert_eq!(
            para_content[3]["type"], "hardBreak",
            "node[3] must be hardBreak (not raw \\n in text): {para_content:?}"
        );
        // File-wide invariant: no raw \n in any text node.
        for node in para_content {
            if node["type"] == "text" {
                let t = node["text"].as_str().unwrap_or("");
                assert!(
                    !t.contains('\n'),
                    "text node must not contain raw \\n: {node:?}"
                );
            }
        }

        // (b) Round-trip regression guard.
        let rendered = adf_to_text(&adf).unwrap();
        assert_eq!(
            rendered, input,
            "multi-line block HTML must round-trip byte-identically (LF-only, non-whitespace final line): rendered={rendered:?}"
        );
    }

    /// BC-7.2.011 EC-3 DOCUMENT-AS-IS (AC-005 test 2) — a standalone HTML comment
    /// block `<!-- x -->` is preserved as visible literal text in a single `text`
    /// node with no `hardBreak` nodes (single line, no interior newlines).
    ///
    /// This is a DOCUMENT-AS-IS pin, not a positive design claim: comment-display
    /// intent is out of scope for this fix. The test registers current behavior
    /// and must remain GREEN against both pre-#492 and post-#492 code.
    ///
    /// NOT a RED GATE test (single-line behavior unchanged by #492).
    #[test]
    fn test_block_html_comment_only_behavior() {
        // BC-7.2.011 EC-3. Comment is preserved verbatim; trailing newline trimmed.
        let adf = markdown_to_adf("<!-- x -->").unwrap();
        let content = adf["content"].as_array().unwrap();
        assert_eq!(
            content.len(),
            1,
            "comment block must produce exactly one paragraph: {adf}"
        );
        let para_content = content[0]["content"].as_array().unwrap();
        // Single line → single text node, no hardBreaks.
        assert_eq!(
            para_content.len(),
            1,
            "comment with no interior newlines must produce exactly one text node: {para_content:?}"
        );
        assert_eq!(
            para_content[0]["type"], "text",
            "comment node must be text type: {para_content:?}"
        );
        assert_eq!(
            para_content[0]["text"], "<!-- x -->",
            "comment text must be preserved verbatim: {para_content:?}"
        );
        // No hardBreak nodes in the paragraph content.
        for node in para_content {
            assert_ne!(
                node["type"], "hardBreak",
                "single-line comment block must not contain hardBreak: {node:?}"
            );
        }
    }

    /// BC-7.2.011 EC-4 (AC-005 test 3) — bare URL at a valid autolink boundary
    /// inside block HTML receives a `link` mark from the `autolink_bare_urls`
    /// post-pass; a URL flush against `"` (href-attribute form) does NOT receive
    /// a link mark.
    ///
    /// Assertions:
    /// (a) `<div>see https://example.com</div>` — `https://example.com` is at a
    ///     valid boundary (preceded by space) and gets a `link` mark on its
    ///     text node (or the node containing it has a link mark).
    /// (b) `<a href="https://example.com">` — URL is flush against `"` (a
    ///     non-boundary character per #473 rules) and stays a plain text node.
    ///
    /// RED GATE: Part (a) relies on the autolink pass seeing per-line text nodes
    /// (produced by Algorithm B). Against pre-#492 code the whole block is one
    /// text node — the autolink pass can still split it, but the test structure
    /// asserts the post-split shape which matches the post-#492 paragraph layout.
    /// Part (b) is a regression guard and passes against both pre- and post-#492.
    #[test]
    fn test_block_html_bare_url_gets_link_mark() {
        // BC-7.2.011 EC-4.

        // (a) Boundary-form: URL preceded by space → gets link mark.
        let adf_boundary = markdown_to_adf("<div>see https://example.com</div>").unwrap();
        let para = &adf_boundary["content"][0];
        assert_eq!(
            para["type"], "paragraph",
            "boundary-form must produce a paragraph: {adf_boundary}"
        );
        let para_nodes = para["content"].as_array().unwrap();
        // After Algorithm B + autolink pass, the URL-bearing line is split.
        // Find the node(s) carrying the URL and assert at least one has a link mark.
        let has_link_mark = para_nodes.iter().any(|node| {
            if node["type"] != "text" {
                return false;
            }
            let text_val = node["text"].as_str().unwrap_or("");
            if !text_val.contains("https://example.com") {
                return false;
            }
            // The node must carry a link mark.
            node.get("marks")
                .and_then(|m| m.as_array())
                .is_some_and(|marks| marks.iter().any(|m| m["type"] == "link"))
        });
        assert!(
            has_link_mark,
            "URL at valid autolink boundary must get a link mark: {para_nodes:?}"
        );

        // (b) href-attribute form: URL flush against `"` → NOT autolinked.
        let adf_href = markdown_to_adf("<a href=\"https://example.com\">").unwrap();
        let para_href = &adf_href["content"][0];
        assert_eq!(
            para_href["type"], "paragraph",
            "href-form must produce a paragraph: {adf_href}"
        );
        let href_nodes = para_href["content"].as_array().unwrap();
        // None of the text nodes containing the URL should carry a link mark.
        let has_unwanted_link = href_nodes.iter().any(|node| {
            if node["type"] != "text" {
                return false;
            }
            let text_val = node["text"].as_str().unwrap_or("");
            if !text_val.contains("https://example.com") {
                return false;
            }
            node.get("marks")
                .and_then(|m| m.as_array())
                .is_some_and(|marks| marks.iter().any(|m| m["type"] == "link"))
        });
        assert!(
            !has_unwanted_link,
            "URL flush against '\"' (href-attribute form) must NOT get a link mark: {href_nodes:?}"
        );
    }

    /// BC-7.2.011 EC-4 (F-P1-002) — URL on an interior line of a multi-line block-HTML
    /// element: the autolink post-pass splits the middle text node into `[pre, link, post]`
    /// and the surrounding `hardBreak` nodes must survive at the correct positions.
    ///
    /// Input: `"<div>\nsee https://example.com\n</div>"`.
    /// Algorithm B step 4 produces (before autolink):
    ///   `[text("<div>"), hardBreak, text("see https://example.com"), hardBreak, text("</div>")]`.
    /// After `autolink_bare_urls` splits the middle text node the expected structure is:
    ///   `[text("<div>"), hardBreak, text("see "), link("https://example.com"), hardBreak, text("</div>")]`.
    ///
    /// Assertions:
    ///   - No text node in the paragraph contains a raw `\n`.
    ///   - Exactly one node carries a `link` mark and its text is `"https://example.com"`.
    ///   - The `hardBreak` at index 1 and the `hardBreak` at index 4 are present (flanking
    ///     the split triple at indices 2–3 and the post-URL text at index 5 respectively).
    #[test]
    fn test_block_html_interior_line_url_split_preserves_hardbreaks() {
        // BC-7.2.011 EC-4 (F-P1-002).
        // Use the markdown_to_adf entry point (same as test_block_html_bare_url_gets_link_mark)
        // so the full Algorithm B + autolink pipeline is exercised.
        let adf = markdown_to_adf("<div>\nsee https://example.com\n</div>").unwrap();
        let para = &adf["content"][0];
        assert_eq!(
            para["type"], "paragraph",
            "multi-line block HTML must produce a paragraph: {adf}"
        );
        let nodes = para["content"]
            .as_array()
            .expect("paragraph must have content array");

        // 1. No text node may contain a raw `\n`.
        for node in nodes {
            if node["type"] == "text" {
                let t = node["text"].as_str().unwrap_or("");
                assert!(
                    !t.contains('\n'),
                    "text node must not contain raw '\\n'; got: {t:?}"
                );
            }
        }

        // 2. Exactly one node must carry a `link` mark with href "https://example.com".
        let link_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| {
                n["type"] == "text"
                    && n.get("marks")
                        .and_then(|m| m.as_array())
                        .is_some_and(|marks| marks.iter().any(|m| m["type"] == "link"))
            })
            .collect();
        assert_eq!(
            link_nodes.len(),
            1,
            "expected exactly one link-marked node; got {}: {nodes:?}",
            link_nodes.len()
        );
        assert_eq!(
            link_nodes[0]["text"].as_str().unwrap_or(""),
            "https://example.com",
            "link-marked node text must be the bare URL"
        );

        // 3. The hardBreak at index 1 (between "<div>" and "see ") must be present,
        //    and a hardBreak must be present at index 4 (between the URL node and "</div>").
        //    Expected full content: [text("<div>"), hb, text("see "), link(url), hb, text("</div>")]
        assert_eq!(
            nodes.len(),
            6,
            "expected 6 nodes after URL split; got {}: {nodes:?}",
            nodes.len()
        );
        assert_eq!(
            nodes[0]["type"], "text",
            "node[0] must be text('<div>'): {nodes:?}"
        );
        assert_eq!(nodes[0]["text"], "<div>", "node[0] text must be '<div>'");
        assert_eq!(
            nodes[1]["type"], "hardBreak",
            "node[1] must be hardBreak (before interior line): {nodes:?}"
        );
        assert_eq!(
            nodes[2]["type"], "text",
            "node[2] must be text('see '): {nodes:?}"
        );
        assert_eq!(nodes[2]["text"], "see ", "node[2] text must be 'see '");
        assert_eq!(
            nodes[3]["type"], "text",
            "node[3] must be the link text node: {nodes:?}"
        );
        assert_eq!(
            nodes[4]["type"], "hardBreak",
            "node[4] must be hardBreak (after URL, before closing tag): {nodes:?}"
        );
        assert_eq!(
            nodes[5]["type"], "text",
            "node[5] must be text('</div>'): {nodes:?}"
        );
        assert_eq!(nodes[5]["text"], "</div>", "node[5] text must be '</div>'");
    }

    /// BC-7.2.011 EC-1 (AC-005 test 4) — CRLF interior and trailing newlines are
    /// normalized to LF; the resulting content array has three text nodes separated
    /// by two hardBreaks, with zero `\r` characters in any text node.
    ///
    /// Input: `"<div>\r\n  x\r\n</div>"`.
    /// Step 2 trims trailing (no trailing `\r\n` after `</div>`).
    /// Step 3 normalizes `\r\n` → `\n` → `"<div>\n  x\n</div>"`.
    /// Split on `\n` → `["<div>", "  x", "</div>"]` (3 segments, 2 boundaries).
    /// Expected: `[text("<div>"), hardBreak, text("  x"), hardBreak, text("</div>")]`.
    ///
    /// RED GATE: fails against pre-#492 handler which does not normalize CRLF and
    /// would preserve `\r` inside the text node.
    #[test]
    fn test_block_html_crlf_interior_no_dangling_cr() {
        // BC-7.2.011 EC-1.
        // Note: we pass the string directly to markdown_to_adf; pulldown-cmark
        // normalizes CRLF to LF before tokenization (CommonMark §2.3), so this
        // test goes through the parser CRLF-normalized path. The handler-level
        // contract (step-3 normalize) is independently exercised by the
        // test_block_html_lone_cr_interior_produces_single_hardbreak handler test.
        // Use a raw block string so CRLF bytes survive the Rust string literal.
        let input = "<div>\r\n  x\r\n</div>";
        let adf = markdown_to_adf(input).unwrap();
        let content = adf["content"].as_array().unwrap();
        assert_eq!(
            content.len(),
            1,
            "CRLF block HTML must be one paragraph: {adf}"
        );
        let para_content = content[0]["content"].as_array().unwrap();
        // Must be [text("<div>"), hardBreak, text("  x"), hardBreak, text("</div>")].
        assert_eq!(
            para_content.len(),
            5,
            "CRLF input (3 segments) must produce 5 content nodes: {para_content:?}"
        );
        assert_eq!(
            para_content[0]["type"], "text",
            "node[0] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[0]["text"], "<div>",
            "node[0] text must be '<div>': {para_content:?}"
        );
        assert_eq!(
            para_content[1]["type"], "hardBreak",
            "node[1] must be hardBreak: {para_content:?}"
        );
        assert_eq!(
            para_content[2]["type"], "text",
            "node[2] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[2]["text"], "  x",
            "node[2] text must be '  x': {para_content:?}"
        );
        assert_eq!(
            para_content[3]["type"], "hardBreak",
            "node[3] must be hardBreak: {para_content:?}"
        );
        assert_eq!(
            para_content[4]["type"], "text",
            "node[4] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[4]["text"], "</div>",
            "node[4] text must be '</div>': {para_content:?}"
        );
        // No \r survives into any text node.
        for node in para_content {
            if node["type"] == "text" {
                let t = node["text"].as_str().unwrap_or("");
                assert!(
                    !t.contains('\r'),
                    "no \\r must survive into any text node after CRLF normalization: {node:?}"
                );
            }
        }
    }

    /// BC-7.2.011 EC-6 (AC-005 test 5) — consecutive blank lines produce double
    /// hardBreak nodes between surrounding content lines.
    ///
    /// Handler-level unit test: constructs the AdfBuilder state directly to
    /// deliver `"<div>\n\na\n</div>"` as the concatenated Html text.
    ///
    /// Note: pulldown-cmark terminates HTML block type 6 at a blank line (CommonMark
    /// §4.6 rule), so `<div>\n\na\n</div>` in a markdown source is parsed as three
    /// separate HTML blocks — the blank line cannot be delivered as interior content
    /// of a single HtmlBlock through the parser. The handler's Algorithm B must
    /// nonetheless handle this case for spec correctness (defense-in-depth), so this
    /// test exercises the algorithm directly by constructing the handler input.
    ///
    /// Step 2 trims trailing `\n` from `</div>\n` → `<div>\n\na\n</div>`.
    /// Step 3: split on `\n` → `["<div>", "", "a", "</div>"]` (4 segments, 3 boundaries).
    /// Algorithm B walk:
    ///   i=0: text("<div>") + hardBreak
    ///   i=1: [empty, no text] + hardBreak
    ///   i=2: text("a") + hardBreak
    ///   i=3: text("</div>") [last, no trailing hardBreak]
    /// Result before step 5b: `[text("<div>"), hb, hb, text("a"), hb, text("</div>")]`.
    /// Step 5b: no leading/trailing hardBreaks — unchanged.
    ///
    /// RED GATE: fails against pre-#492 handler which emits a single text node
    /// containing `"<div>\n\na\n</div>"` (embedded raw newlines, not hardBreaks).
    #[test]
    fn test_block_html_consecutive_blank_lines_produce_double_hardbreak() {
        // BC-7.2.011 EC-6. Handler-level test.
        // pulldown-cmark splits at blank lines (type 6 HTML block rule), so we
        // construct the handler input directly to test Algorithm B's handling of
        // empty interior segments.
        let mut builder = AdfBuilder::new();
        builder.push(NodeKind::HtmlBlock);
        // Simulate the handler receiving the concatenated Html lines including
        // the interior blank line. The trailing "\n" is present (as pulldown emits).
        builder.push_text("<div>\n\na\n</div>\n");
        builder.end(TagEnd::HtmlBlock);

        let root = &builder.root;
        assert_eq!(
            root.len(),
            1,
            "consecutive-blank-line block HTML must produce one paragraph: {root:?}"
        );
        let para_content = root[0]["content"].as_array().unwrap();
        // Step 2 trims trailing \n: "<div>\n\na\n</div>".
        // Step 3 split: ["<div>", "", "a", "</div>"] — 4 segments, 3 boundaries.
        // [text("<div>"), hb, hb, text("a"), hb, text("</div>")] — 6 nodes.
        assert_eq!(
            para_content.len(),
            6,
            "4 segments (3 boundaries) must produce 6 content nodes: {para_content:?}"
        );
        assert_eq!(
            para_content[0]["type"], "text",
            "node[0] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[0]["text"], "<div>",
            "node[0] text: {para_content:?}"
        );
        assert_eq!(
            para_content[1]["type"], "hardBreak",
            "node[1] must be hardBreak: {para_content:?}"
        );
        assert_eq!(
            para_content[2]["type"], "hardBreak",
            "node[2] must also be hardBreak (double-hardBreak from empty segment): {para_content:?}"
        );
        assert_eq!(
            para_content[3]["type"], "text",
            "node[3] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[3]["text"], "a",
            "node[3] text must be 'a': {para_content:?}"
        );
        assert_eq!(
            para_content[4]["type"], "hardBreak",
            "node[4] must be hardBreak: {para_content:?}"
        );
        assert_eq!(
            para_content[5]["type"], "text",
            "node[5] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[5]["text"], "</div>",
            "node[5] text must be '</div>': {para_content:?}"
        );
    }

    /// BC-7.2.011 EC-8 (AC-005 test 6) — a block whose first line is blank
    /// produces NO leading `hardBreak` in the final content array.
    ///
    /// Algorithm B produces a leading `hardBreak` from the empty leading segment;
    /// step 5b (`trim_leading_trailing_hardbreaks`) removes it.
    ///
    /// Handler-level unit test: constructs the AdfBuilder state directly to
    /// deliver `"\n<div>x</div>\n"` as the concatenated Html input (one child
    /// text node with that exact content), bypassing pulldown-cmark (which
    /// normalizes line endings before tokenization and may not deliver a leading
    /// blank line in practice — defense-in-depth per BC-7.2.011 EC-8 note).
    ///
    /// RED GATE: fails against pre-#492 handler, which does not apply
    /// `trim_leading_trailing_hardbreaks` and would leave a leading hardBreak
    /// (or, with the current buggy single-text-node implementation, emits the
    /// raw `\n<div>x</div>` with a leading `\n` in the text field).
    #[test]
    fn test_block_html_leading_blank_line_no_leading_hardbreak() {
        // BC-7.2.011 EC-8. Handler-level test: simulate pulldown delivering
        // "\n<div>x</div>\n" as the concatenated Html text for one HtmlBlock.
        // This is equivalent to a block whose content is: blank line, then
        // "<div>x</div>", then a trailing newline.
        let mut builder = AdfBuilder::new();
        // Simulate Start(HtmlBlock):
        builder.push(NodeKind::HtmlBlock);
        // Simulate Event::Html lines: the whole block as one concatenated string
        // (pulldown emits per-line Html events; each appends to the stack top via
        // push_text; together they make the children that end() processes).
        builder.push_text("\n<div>x</div>\n");
        // Simulate End(HtmlBlock):
        builder.end(TagEnd::HtmlBlock);

        // The result should be in builder.root.
        let root = &builder.root;
        assert_eq!(
            root.len(),
            1,
            "leading-blank-line block HTML must produce one paragraph: {root:?}"
        );
        let para_content = root[0]["content"].as_array().unwrap();
        // After step 5b trim: [text("<div>x</div>")] — the leading hardBreak removed.
        assert!(
            !para_content.is_empty(),
            "content must not be empty: {para_content:?}"
        );
        assert_eq!(
            para_content[0]["type"], "text",
            "first content node must be text (leading hardBreak must be trimmed by step 5b): {para_content:?}"
        );
        assert_ne!(
            para_content[0]["type"], "hardBreak",
            "content must NOT begin with hardBreak after step 5b trim: {para_content:?}"
        );
        // Confirm no \n in the text node.
        let first_text = para_content[0]["text"].as_str().unwrap_or("");
        assert!(
            !first_text.contains('\n'),
            "text node must not contain raw \\n: {para_content:?}"
        );
    }

    /// BC-7.2.011 EC-9 (AC-005 test 7) — a lone `\r` (old-Mac line ending) is
    /// treated as a single line-break boundary, producing exactly ONE `hardBreak`
    /// node, and no `\r` character survives into any text node.
    ///
    /// Handler-level unit test: constructs the AdfBuilder state directly to
    /// deliver `"<div>\rx</div>"` as the concatenated Html input, bypassing
    /// pulldown-cmark (which normalizes CR to LF before tokenization per
    /// CommonMark §2.3 — so a lone `\r` in the handler input is a
    /// defense-in-depth path, not a routine parser-produced scenario).
    ///
    /// RED GATE: fails against pre-#492 handler because the handler does not
    /// normalize lone `\r` — the current handler strips only a trailing `\n`
    /// and would embed the `\r` inside the text node verbatim.
    #[test]
    fn test_block_html_lone_cr_interior_produces_single_hardbreak() {
        // BC-7.2.011 EC-9. Handler-level test: simulate pulldown delivering
        // "<div>\rx</div>" (lone CR, old-Mac line ending) as the Html text.
        let mut builder = AdfBuilder::new();
        builder.push(NodeKind::HtmlBlock);
        builder.push_text("<div>\rx</div>");
        builder.end(TagEnd::HtmlBlock);

        let root = &builder.root;
        assert_eq!(
            root.len(),
            1,
            "lone-CR block HTML must produce one paragraph: {root:?}"
        );
        let para_content = root[0]["content"].as_array().unwrap();
        // Step 3 normalizes lone `\r` → `\n`, split → ["<div>", "x</div>"].
        // Expected: [text("<div>"), hardBreak, text("x</div>")] — 3 nodes.
        assert_eq!(
            para_content.len(),
            3,
            "lone-CR input (2 segments) must produce 3 content nodes: {para_content:?}"
        );
        assert_eq!(
            para_content[0]["type"], "text",
            "node[0] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[0]["text"], "<div>",
            "node[0] text must be '<div>': {para_content:?}"
        );
        assert_eq!(
            para_content[1]["type"], "hardBreak",
            "node[1] must be exactly ONE hardBreak (lone \\r = one boundary): {para_content:?}"
        );
        assert_eq!(
            para_content[2]["type"], "text",
            "node[2] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[2]["text"], "x</div>",
            "node[2] text must be 'x</div>': {para_content:?}"
        );
        // No \r must survive in any text node.
        for node in para_content {
            if node["type"] == "text" {
                let t = node["text"].as_str().unwrap_or("");
                assert!(
                    !t.contains('\r'),
                    "no \\r must survive after lone-CR normalization: {node:?}"
                );
            }
        }
    }

    /// BC-7.2.011 EC-10 (AC-005 test 8) — trailing-whitespace final line (LF-only,
    /// document-final):
    ///   - Forward ADF preserves `[text("<div>x</div>"), hardBreak, text("   ")]`
    ///     (step 2 only trims `\r`/`\n`, NOT spaces/tabs).
    ///   - Round-trip via `adf_to_text` yields `"<div>x</div>"` (the trailing-
    ///     whitespace line AND its `hardBreak` separator are stripped by
    ///     `AdfRenderer::finish()`'s document-global `trim_end()`).
    ///   - NOT byte-identical.
    ///
    /// Handler-level unit test: delivers `"<div>x</div>\n   "` as the Html text
    /// directly (bypassing pulldown-cmark, which may normalize or strip trailing
    /// whitespace lines during HTML-block tokenization — defense-in-depth per
    /// BC-7.2.011 EC-10 note).
    ///
    /// RED GATE: fails against pre-#492 handler, which cannot produce the split
    /// `[text, hardBreak, text("   ")]` shape — it emits one text node and would
    /// not produce the correct forward ADF assertion.
    #[test]
    fn test_block_html_trailing_whitespace_final_line_not_byte_identical() {
        // BC-7.2.011 EC-10. Handler-level test.
        // Input: "<div>x</div>\n   " (LF-only; final line is three spaces).
        // Step 2: trim \r/\n only → "<div>x</div>\n   " (spaces preserved).
        // Step 3: normalize (no CRLF) → split on \n → ["<div>x</div>", "   "].
        // Step 4: [text("<div>x</div>"), hardBreak, text("   ")] — 3 nodes.
        let mut builder = AdfBuilder::new();
        builder.push(NodeKind::HtmlBlock);
        builder.push_text("<div>x</div>\n   ");
        builder.end(TagEnd::HtmlBlock);

        let root = &builder.root;
        assert_eq!(
            root.len(),
            1,
            "trailing-whitespace block HTML must produce one paragraph: {root:?}"
        );
        let para_content = root[0]["content"].as_array().unwrap();
        // Forward ADF: [text("<div>x</div>"), hardBreak, text("   ")].
        assert_eq!(
            para_content.len(),
            3,
            "2 segments must produce 3 content nodes (text, hardBreak, text): {para_content:?}"
        );
        assert_eq!(
            para_content[0]["type"], "text",
            "node[0] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[0]["text"], "<div>x</div>",
            "node[0] text: {para_content:?}"
        );
        assert_eq!(
            para_content[1]["type"], "hardBreak",
            "node[1] must be hardBreak: {para_content:?}"
        );
        assert_eq!(
            para_content[2]["type"], "text",
            "node[2] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[2]["text"], "   ",
            "node[2] must be the trailing-whitespace text '   ' (step 2 must NOT strip non-newline whitespace): {para_content:?}"
        );

        // Round-trip via adf_to_text: AdfRenderer::finish() trim_end() strips
        // the trailing-whitespace line and its hardBreak — NOT byte-identical.
        // Wrap the paragraph into a proper ADF doc for adf_to_text.
        let doc = json!({
            "version": 1,
            "type": "doc",
            "content": root,
        });
        let round_tripped = adf_to_text(&doc).unwrap();
        assert_eq!(
            round_tripped, "<div>x</div>",
            "round-trip must strip the trailing-whitespace line via finish().trim_end(): round_tripped={round_tripped:?}"
        );
        // Confirm NOT byte-identical to original handler input.
        assert_ne!(
            round_tripped, "<div>x</div>\n   ",
            "round-trip must NOT be byte-identical to trailing-whitespace input: {round_tripped:?}"
        );
    }

    /// BC-7.2.011 EC-7 — an all-whitespace / empty block HTML body produces NO
    /// paragraph node (`EndResult::Empty`, step-6 early-return).
    ///
    /// The step-6 guard is the operative prune path for `paragraph` because
    /// `paragraph` is excluded from `is_empty_block_container` (empty paragraphs
    /// are valid ADF for other node types, but here the early-return prevents an
    /// empty paragraph being emitted at all). Without the guard an empty
    /// `{"type":"paragraph","content":[]}` would reach the wire and Jira would
    /// reject it with HTTP 400.
    ///
    /// Handler-level unit test: constructs `AdfBuilder` state directly to deliver
    /// whitespace-only text (newlines only) as the `HtmlBlock` body, then asserts
    /// that `builder.root` remains empty — no paragraph node is appended.
    ///
    /// RED GATE: fails against any handler that does not implement the step-6
    /// early-return (would instead append an empty `paragraph` to `builder.root`).
    #[test]
    fn test_block_html_all_empty_block_emits_no_paragraph() {
        // BC-7.2.011 EC-7. Handler-level test: whitespace-only Html content.
        // Step 2: trim_end_matches(['\n', '\r']) on "\n\n\n" → "" (empty).
        // Step 3: normalize → split → [""] — one segment, all empty.
        // Step 4: no non-empty segment → content Vec is empty (or only hardBreaks
        //         from boundaries, which step 5 would trim).
        // Step 6: early-return EndResult::Empty — nothing pushed to builder.root.
        let mut builder = AdfBuilder::new();
        builder.push(NodeKind::HtmlBlock);
        builder.push_text("\n\n\n");
        builder.end(TagEnd::HtmlBlock);

        let root = &builder.root;
        assert!(
            root.is_empty(),
            "all-whitespace block HTML must emit no paragraph (step-6 early-return); \
             builder.root must be empty but got: {root:?}"
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
        let adf = markdown_to_adf("**line one  \nline two**").unwrap();

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

    // --- F-PASS4-C1: document-order preservation for hoisted blocks --------
    // When a hoisted block from an empty-bodied task item precedes a real task
    // item in SOURCE ORDER, the output must preserve that order (hoisted block
    // BEFORE the taskList that contains the following task items).
    //
    // These tests assert the EXACT top-level sequence and MUST FAIL before the
    // order-preserving reclassification fix and PASS after.
    //
    // Order invariant: the output sequence mirrors source order; a hoisted block
    // that appeared BEFORE a task item in source order must appear before the
    // taskList containing that task item in the output.

    #[test]
    fn test_order_preserving_hoist_empty_task_then_plain_then_real_task() {
        // F-PASS4-C1 trigger: `- [ ]\n  - plain inner\n- [x] after`
        // Source order: plain inner (item1 nested content), after (item2 task).
        // Expected output doc-level: [bulletList(plain inner), taskList([after])]
        // (NOT [taskList([after]), bulletList(plain inner)] — that is inverted.)
        let md = "- [ ]\n  - plain inner\n- [x] after\n";
        let adf = markdown_to_adf(md).unwrap();
        let serialized = serde_json::to_string_pretty(&adf).unwrap();

        assert_valid_adf_structure(&adf);

        let doc_children = adf["content"].as_array().expect("doc must have content");
        assert!(
            doc_children.len() >= 2,
            "expected at least 2 top-level nodes but got {}: {}",
            doc_children.len(),
            serialized
        );

        // First node must be bulletList (the hoisted plain sublist from item1)
        assert_eq!(
            doc_children[0]["type"].as_str(),
            Some("bulletList"),
            "doc[0] must be bulletList (source order: plain before task) but got '{}': {}",
            doc_children[0]["type"].as_str().unwrap_or("?"),
            serialized
        );

        // Second node must be taskList (containing the 'after' item from item2)
        assert_eq!(
            doc_children[1]["type"].as_str(),
            Some("taskList"),
            "doc[1] must be taskList but got '{}': {}",
            doc_children[1]["type"].as_str().unwrap_or("?"),
            serialized
        );

        // The taskList must contain 'after'
        let task_items = doc_children[1]["content"]
            .as_array()
            .expect("taskList must have content");
        let text = task_items[0]["content"][0]["text"].as_str().unwrap_or("");
        assert_eq!(
            text, "after",
            "taskList must contain 'after' text: {}",
            serialized
        );
    }

    #[test]
    fn test_order_preserving_hoist_real_task_then_empty_then_real_task() {
        // F-PASS4-C1: `- [x] before\n- [ ]\n  - plain\n- [x] after`
        // Source order: before (task), plain (hoisted nested content), after (task).
        //
        // Schema-valid order-preserving shape chosen:
        //   [taskList([before]), bulletList([plain]), taskList([after])]
        //
        // Splitting the taskList around the interposed bulletList is the only
        // way to preserve source order with valid ADF. A single merged taskList
        // would require reordering 'before' and 'after' (skipping 'plain')
        // which violates the order invariant.
        //
        // BC back-propagation note: BC-7.2.010 does not specify this interleaving
        // shape. The implemented invariant is: output preserves source document
        // order; valid ADF; does not drop content. When task items and hoisted
        // blocks are interleaved, one taskList node is emitted per contiguous run
        // of task items, each run separated by the interposed hoisted block(s).
        let md = "- [x] before\n- [ ]\n  - plain\n- [x] after\n";
        let adf = markdown_to_adf(md).unwrap();
        let serialized = serde_json::to_string_pretty(&adf).unwrap();

        assert_valid_adf_structure(&adf);

        let doc_children = adf["content"].as_array().expect("doc must have content");
        assert_eq!(
            doc_children.len(),
            3,
            "expected [taskList(before), bulletList(plain), taskList(after)] but got {}: {}",
            doc_children.len(),
            serialized
        );

        // [0]: taskList with 'before'
        assert_eq!(
            doc_children[0]["type"].as_str(),
            Some("taskList"),
            "doc[0] must be taskList (before): {}",
            serialized
        );
        let before_text = doc_children[0]["content"][0]["content"][0]["text"]
            .as_str()
            .unwrap_or("");
        assert_eq!(
            before_text, "before",
            "doc[0] taskList must contain 'before': {}",
            serialized
        );

        // [1]: bulletList with 'plain'
        assert_eq!(
            doc_children[1]["type"].as_str(),
            Some("bulletList"),
            "doc[1] must be bulletList (plain): {}",
            serialized
        );

        // [2]: taskList with 'after'
        assert_eq!(
            doc_children[2]["type"].as_str(),
            Some("taskList"),
            "doc[2] must be taskList (after): {}",
            serialized
        );
        let after_text = doc_children[2]["content"][0]["content"][0]["text"]
            .as_str()
            .unwrap_or("");
        assert_eq!(
            after_text, "after",
            "doc[2] taskList must contain 'after': {}",
            serialized
        );
    }

    #[test]
    fn test_order_preserving_hoist_empty_task_two_nested_task_items() {
        // F-PASS4-C1: `- [ ]\n  - [x] a\n  - [ ] b`
        // Empty parent task, TWO nested task items. Both must survive in order.
        // Expected: doc > taskList{taskItem(a), taskItem(b)}
        let md = "- [ ]\n  - [x] a\n  - [ ] b\n";
        let adf = markdown_to_adf(md).unwrap();
        let serialized = serde_json::to_string_pretty(&adf).unwrap();

        assert_valid_adf_structure(&adf);

        let doc_children = adf["content"].as_array().expect("doc must have content");
        assert_eq!(
            doc_children.len(),
            1,
            "expected 1 top-level taskList but got {}: {}",
            doc_children.len(),
            serialized
        );

        let task_list = &doc_children[0];
        assert_eq!(
            task_list["type"].as_str(),
            Some("taskList"),
            "doc[0] must be taskList: {}",
            serialized
        );

        let items = task_list["content"]
            .as_array()
            .expect("taskList must have content");
        assert_eq!(
            items.len(),
            2,
            "taskList must have 2 items (a, b) but got {}: {}",
            items.len(),
            serialized
        );

        // Item order: a first, b second
        let text_a = items[0]["content"][0]["text"].as_str().unwrap_or("");
        assert_eq!(text_a, "a", "first taskItem must be 'a': {}", serialized);
        let text_b = items[1]["content"][0]["text"].as_str().unwrap_or("");
        assert_eq!(text_b, "b", "second taskItem must be 'b': {}", serialized);
    }

    #[test]
    fn test_order_preserving_hoist_real_task_then_empty_with_nested_task() {
        // F-PASS4-C1: `- [x] real\n- [ ]\n  - [ ] nested`
        // Source order: real (task), nested (from empty parent's sublist).
        // Expected: real appears before nested in output.
        //
        // Shape: [taskList([real, nested])] if the nested sublist reclassifies
        // to a task run that can be appended, OR
        // [taskList([real]), taskList([nested])] if split across the empty item.
        //
        // The key invariant: 'real' must appear at or before 'nested' in the
        // output — NOT after.
        let md = "- [x] real\n- [ ]\n  - [ ] nested\n";
        let adf = markdown_to_adf(md).unwrap();
        let serialized = serde_json::to_string_pretty(&adf).unwrap();

        assert_valid_adf_structure(&adf);

        let doc_children = adf["content"].as_array().expect("doc must have content");
        assert!(
            !doc_children.is_empty(),
            "doc must have content: {}",
            serialized
        );

        // Find 'real' and 'nested' positions in the flattened task item sequence.
        // Walk all taskList nodes in order and collect taskItem text values.
        fn collect_task_item_texts(node: &Value, out: &mut Vec<String>) {
            let ty = node["type"].as_str().unwrap_or("");
            if ty == "taskItem" {
                let text = node["content"][0]["text"].as_str().unwrap_or("");
                out.push(text.to_owned());
            }
            if let Some(children) = node["content"].as_array() {
                for child in children {
                    collect_task_item_texts(child, out);
                }
            }
        }
        let mut texts = Vec::new();
        for child in doc_children {
            collect_task_item_texts(child, &mut texts);
        }
        let real_pos = texts.iter().position(|t| t == "real");
        let nested_pos = texts.iter().position(|t| t == "nested");
        assert!(
            real_pos.is_some(),
            "'real' task item must appear in output: {}",
            serialized
        );
        assert!(
            nested_pos.is_some(),
            "'nested' task item must appear in output: {}",
            serialized
        );
        assert!(
            real_pos.unwrap() < nested_pos.unwrap(),
            "'real' (pos {}) must come before 'nested' (pos {}) in output: {}",
            real_pos.unwrap(),
            nested_pos.unwrap(),
            serialized
        );
    }

    // --- F-PASS4-I1: empty-list check in assert_valid_adf_structure ---------
    // Verify the strengthened validator rejects empty list nodes directly
    // (not just via the separate assert_no_empty_list_content path).

    #[test]
    fn test_validator_rejects_empty_bullet_list() {
        // F-PASS4-I1: assert_valid_adf_structure must reject empty bulletList.
        let bad = serde_json::json!({
            "type": "doc",
            "content": [{ "type": "bulletList", "content": [] }]
        });
        let result = std::panic::catch_unwind(|| assert_valid_adf_structure(&bad));
        assert!(
            result.is_err(),
            "assert_valid_adf_structure must panic on empty bulletList"
        );
    }

    #[test]
    fn test_validator_rejects_empty_task_list() {
        // F-PASS4-I1: assert_valid_adf_structure must reject empty taskList.
        let bad = serde_json::json!({
            "type": "doc",
            "content": [{ "type": "taskList", "content": [] }]
        });
        let result = std::panic::catch_unwind(|| assert_valid_adf_structure(&bad));
        assert!(
            result.is_err(),
            "assert_valid_adf_structure must panic on empty taskList"
        );
    }

    // --- F-PASS4-I2: allowlist-based validator arms --------------------------
    // The listItem, blockquote, and panel arms must use allowlists (not denylists)
    // so illegal direct children are caught even if not previously anticipated.

    #[test]
    fn test_validator_rejects_heading_inside_list_item() {
        // F-PASS4-I2: listItem may NOT contain heading as a direct child.
        // (allowlist: paragraph, bulletList, orderedList, codeBlock, mediaSingle)
        let bad = serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "bulletList",
                "content": [{
                    "type": "listItem",
                    "content": [{ "type": "heading", "attrs": { "level": 1 }, "content": [{ "type": "text", "text": "h" }] }]
                }]
            }]
        });
        let result = std::panic::catch_unwind(|| assert_valid_adf_structure(&bad));
        assert!(
            result.is_err(),
            "assert_valid_adf_structure must panic on heading inside listItem"
        );
    }

    #[test]
    fn test_validator_rejects_table_inside_blockquote() {
        // F-PASS4-I2: blockquote may NOT contain table as a direct child.
        // (allowlist: paragraph, heading, bulletList, orderedList, taskList,
        //  codeBlock, rule, mediaSingle, blockquote — NOT table)
        let bad = serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "blockquote",
                "content": [{
                    "type": "table",
                    "content": []
                }]
            }]
        });
        let result = std::panic::catch_unwind(|| assert_valid_adf_structure(&bad));
        assert!(
            result.is_err(),
            "assert_valid_adf_structure must panic on table inside blockquote"
        );
    }

    #[test]
    fn test_validator_rejects_blockquote_inside_panel() {
        // F-PASS4-I2: panel may NOT contain blockquote as a direct child.
        // (allowlist: paragraph, heading, bulletList, orderedList, taskList,
        //  codeBlock, rule, mediaSingle — NOT blockquote, panel, table)
        let bad = serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "panel",
                "attrs": { "panelType": "info" },
                "content": [{
                    "type": "blockquote",
                    "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "x" }] }]
                }]
            }]
        });
        let result = std::panic::catch_unwind(|| assert_valid_adf_structure(&bad));
        assert!(
            result.is_err(),
            "assert_valid_adf_structure must panic on blockquote inside panel"
        );
    }

    // --- F-1 (F5-pass6): loose task item with nested sublist preserves doc order ---
    // Mirror of the already-fixed tight-path bug (F-PASS4-C1).
    // Before the fix, the loose branch called append_child(hoist) BEFORE returning
    // Single(taskItem), so BulletList children became [bulletList(inner), taskItem(outer)]
    // → reclassified as [bulletList(inner), taskList(outer)] — inverted order.

    #[test]
    fn test_loose_task_item_with_nested_sublist_preserves_order() {
        // F-1 regression: `- [ ] outer\n\n  - inner`
        // A loose task item (blank-line-separated) with a nested plain sub-list.
        // Expected doc-level order: [taskList(outer), bulletList(inner)]
        // NOT [bulletList(inner), taskList(outer)].
        let md = "- [ ] outer\n\n  - inner\n";
        let adf = markdown_to_adf(md).unwrap();

        assert_valid_adf_structure(&adf);

        let doc_children = adf["content"].as_array().expect("doc must have content");
        assert!(
            !doc_children.is_empty(),
            "doc must have at least one child: {adf}"
        );
        // The taskList must come FIRST (outer item).
        assert_eq!(
            doc_children[0]["type"], "taskList",
            "first doc child must be taskList (outer), got doc: {adf}"
        );
        // The taskList must contain the outer taskItem.
        let task_items = doc_children[0]["content"]
            .as_array()
            .expect("taskList content");
        assert_eq!(
            task_items[0]["type"], "taskItem",
            "taskList first child must be taskItem: {adf}"
        );
        let task_text = serde_json::to_string(&task_items[0]).unwrap_or_default();
        assert!(
            task_text.contains("outer"),
            "taskItem must contain 'outer' text: {adf}"
        );
        // The bulletList must come AFTER (nested sub-list hoisted to sibling).
        if doc_children.len() >= 2 {
            assert_eq!(
                doc_children[1]["type"], "bulletList",
                "second doc child must be bulletList (inner), got: {adf}"
            );
            let list_text = serde_json::to_string(&doc_children[1]).unwrap_or_default();
            assert!(
                list_text.contains("inner"),
                "bulletList must contain 'inner' text: {adf}"
            );
        }
    }

    #[test]
    fn test_loose_task_item_with_nested_task_sublist_preserves_order() {
        // F-1 regression (task sublist variant): `- [ ] outer\n\n  - [x] inner`
        // A loose task item with a nested TASK sub-list.
        // Per EC-13: nested taskList is a sibling within the parent taskList.
        // Expected: the outer item and inner item are both in the same taskList
        // (sibling semantics), OR outer taskList comes before inner taskList.
        // Most importantly: the outer task item must NOT appear AFTER the inner list.
        let md = "- [ ] outer\n\n  - [x] inner\n";
        let adf = markdown_to_adf(md).unwrap();

        assert_valid_adf_structure(&adf);

        let doc_children = adf["content"].as_array().expect("doc must have content");
        assert!(!doc_children.is_empty(), "doc must not be empty: {adf}");
        // The first doc child must be a taskList (not bulletList or taskItem).
        assert_eq!(
            doc_children[0]["type"], "taskList",
            "first doc child must be taskList, got: {adf}"
        );
        // The taskList must lead with the outer taskItem (first child is taskItem per schema).
        let task_content = doc_children[0]["content"]
            .as_array()
            .expect("taskList content");
        assert_eq!(
            task_content[0]["type"], "taskItem",
            "taskList first child must be taskItem (outer), not nested list: {adf}"
        );
        let outer_text = serde_json::to_string(&task_content[0]).unwrap_or_default();
        assert!(
            outer_text.contains("outer"),
            "first taskItem must contain 'outer': {adf}"
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
        let out = normalize_panel_content(children, 0).unwrap();
        assert_eq!(out.len(), 1, "paragraph kept: {out:?}");
        assert!(
            out[0].get("marks").is_none(),
            "node-level marks stripped from panel paragraph: {:?}",
            out[0]
        );
        // Inline content (and its marks) is untouched — only node-level marks go.
        assert_eq!(out[0]["content"][0]["text"], "x");
    }

    // --- F-P11-001: empty outer ORDERED task list + nested sublist -----------

    #[test]
    fn test_empty_outer_ordered_task_with_nested_ordered_task_is_valid() {
        // F-P11-001: `1. [ ]\n   1. [x] x`
        // Empty outer ordered task item whose only child is a nested ordered task.
        // After reclassification the inner `orderedList` becomes a `taskList`;
        // the outer `listItem` is pruned (empty body); the stray `taskList` is
        // hoisted. The outer `OrderedList` then sees no `taskItem` child —
        // it must NOT wrap `[taskList]` directly in `orderedList` (invalid ADF).
        // It must dissolve (hoist stray blocks to grandparent) instead.
        let md = "1. [ ]\n   1. [x] x\n";
        let adf = markdown_to_adf(md).unwrap();
        assert_valid_adf_structure(&adf);
        // The nested task item "x" must appear somewhere in the output.
        let s = serde_json::to_string(&adf).unwrap();
        assert!(
            s.contains("\"x\"") || s.contains("x"),
            "content must not be dropped: {adf}"
        );
    }

    #[test]
    fn test_empty_outer_ordered_task_with_nested_plain_list_is_valid() {
        // F-P11-001 variant: `1. [ ]\n   - plain inner`
        let md = "1. [ ]\n   - plain inner\n";
        let adf = markdown_to_adf(md).unwrap();
        assert_valid_adf_structure(&adf);
        let s = serde_json::to_string(&adf).unwrap();
        assert!(
            s.contains("plain") && s.contains("inner"),
            "content must not be dropped: {adf}"
        );
    }

    #[test]
    fn test_empty_outer_ordered_task_with_nested_task_list_is_valid() {
        // F-P11-001 variant: `1. [ ]\n   - [x] nested`
        let md = "1. [ ]\n   - [x] nested\n";
        let adf = markdown_to_adf(md).unwrap();
        assert_valid_adf_structure(&adf);
        let s = serde_json::to_string(&adf).unwrap();
        assert!(s.contains("nested"), "content must not be dropped: {adf}");
    }

    // --- F-PASS13-C1: plain outer item wrapping multi-level nested task list -

    #[test]
    fn test_plain_outer_item_with_multi_level_nested_task_list_is_valid() {
        // F-PASS13-C1: `- outer\n  - [ ] a\n    - [ ] b`
        // Plain (non-task) outer bullet whose body is a task list containing
        // a nested task sublist. The inner `taskList` contains a nested
        // `taskList` child. `normalize_list_item_content` converts the outer
        // taskList → bulletList; during iteration it sees `[taskItem(a), taskList([b])]`.
        // The converted taskList([b]) → bulletList([listItem(b)]) must be nested
        // INSIDE the listItem(a), not appended as a sibling of listItem(a).
        let md = "- outer\n  - [ ] a\n    - [ ] b\n";
        let adf = markdown_to_adf(md).unwrap();
        assert_valid_adf_structure(&adf);
        let s = serde_json::to_string(&adf).unwrap();
        assert!(
            s.contains("\"a\"") || s.contains("\"b\""),
            "content must not be dropped: {adf}"
        );
    }

    #[test]
    fn test_plain_outer_item_with_multi_level_nested_ordered_task_list_is_valid() {
        // F-PASS13-C1 ordered variant: `- outer\n  1. [ ] a\n     1. [ ] b`
        let md = "- outer\n  1. [ ] a\n     1. [ ] b\n";
        let adf = markdown_to_adf(md).unwrap();
        assert_valid_adf_structure(&adf);
    }

    // --- Comprehensive structural-validity corpus (stop the whack-a-mole) ----

    #[test]
    // This test function legitimately exceeds clippy's line limit: it is a single
    // parametric corpus covering the Cartesian product of list kind × tightness ×
    // outer body × nested child × depth × interleaving × container wrapping.
    // Splitting it into smaller tests would lose the combinatorial coverage
    // guarantee and make failures harder to trace across dimensions.
    #[allow(clippy::too_many_lines)]
    fn test_adf_structural_validity_comprehensive_corpus() {
        // Comprehensive corpus covering the cartesian product of:
        //   list kind × tightness × outer body × nested child × depth
        //   × interleaving × container wrapping
        // Every input is checked with assert_valid_adf_structure (panics on
        // invalid ADF), a no-content-drop check (every non-whitespace word
        // appears in the output text), and a no-panic guarantee.
        //
        // Inputs are grouped by dimension for readability. Labels encode the
        // triggering scenario so failures are easy to diagnose.
        let inputs: &[(&str, &str)] = &[
            // ================================================================
            // DIMENSION: list kind × outer body × tightness
            // ================================================================

            // --- Bullet list, tight ---
            ("bullet-tight-task-unchecked", "- [ ] a\n"),
            ("bullet-tight-task-checked", "- [x] a\n"),
            ("bullet-tight-plain", "- plain\n"),
            ("bullet-tight-two-tasks", "- [ ] a\n- [x] b\n"),
            ("bullet-tight-mixed-task-plain", "- [ ] task\n- plain\n"),
            ("bullet-tight-empty-task", "- [ ]\n"),
            // --- Bullet list, loose ---
            ("bullet-loose-task-unchecked", "- [ ] a\n\n- [x] b\n"),
            ("bullet-loose-task-multiline", "- [ ] line1\n\n  line2\n"),
            ("bullet-loose-plain", "- a\n\n- b\n"),
            ("bullet-loose-empty-task", "- [ ]\n\n- [x] b\n"),
            // --- Ordered list, tight ---
            ("ordered-tight-plain", "1. a\n2. b\n"),
            ("ordered-tight-task-unchecked", "1. [ ] a\n"),
            ("ordered-tight-task-checked", "1. [x] a\n"),
            ("ordered-tight-two-tasks", "1. [ ] a\n2. [x] b\n"),
            ("ordered-tight-mixed-task-plain", "1. [ ] a\n2. plain\n"),
            ("ordered-tight-empty-task", "1. [ ]\n"),
            // --- Ordered list, loose ---
            ("ordered-loose-task", "1. [ ] a\n\n2. [x] b\n"),
            ("ordered-loose-plain", "1. a\n\n2. b\n"),
            // ================================================================
            // DIMENSION: nested child type × outer body
            // ================================================================

            // Bullet outer + no nested child (already covered above)

            // --- Bullet outer with task body + nested plain list ---
            (
                "bullet-task-outer-nested-plain",
                "- [ ] outer\n  - plain inner\n",
            ),
            (
                "bullet-task-outer-nested-plain-tight",
                "- [ ] outer\n- [x] after\n",
            ),
            // --- Bullet outer with task body + nested task list ---
            (
                "bullet-task-outer-nested-task",
                "- [ ] outer\n  - [x] nested\n",
            ),
            (
                "bullet-task-outer-nested-task-checked",
                "- [x] outer\n  - [ ] nested\n",
            ),
            // --- Bullet outer with task body + nested mixed ---
            (
                "bullet-task-outer-nested-mixed",
                "- [ ] outer\n  - [x] task sub\n  - plain sub\n",
            ),
            // --- Bullet PLAIN outer with nested task list (F-PASS13-C1 class) ---
            ("bullet-plain-outer-nested-task", "- outer\n  - [ ] a\n"),
            (
                "bullet-plain-outer-nested-task-multiitem",
                "- outer\n  - [ ] a\n  - [x] b\n",
            ),
            (
                "bullet-plain-outer-nested-task-then-plain",
                "- outer\n  - [ ] task\n  - plain\n",
            ),
            // --- Bullet EMPTY outer with nested list (F-PASS3-C1 class) ---
            (
                "bullet-empty-outer-nested-plain",
                "- [ ]\n  - plain inner\n",
            ),
            ("bullet-empty-outer-nested-task", "- [ ]\n  - [x] nested\n"),
            (
                "bullet-empty-outer-nested-mixed",
                "- [ ]\n  - [x] task\n  - plain\n",
            ),
            // --- Ordered outer with task body + nested plain list ---
            (
                "ordered-task-outer-nested-plain",
                "1. [ ] outer\n   - plain inner\n",
            ),
            // --- Ordered outer with task body + nested task list ---
            (
                "ordered-task-outer-nested-task",
                "1. [ ] outer\n   1. [x] nested\n",
            ),
            // --- Ordered PLAIN outer with nested task list ---
            ("ordered-plain-outer-nested-task", "1. outer\n   1. [ ] a\n"),
            // --- Ordered EMPTY outer with nested list (F-P11-001 class) ---
            (
                "ordered-empty-outer-nested-task-ordered",
                "1. [ ]\n   1. [x] x\n",
            ),
            (
                "ordered-empty-outer-nested-plain-bullet",
                "1. [ ]\n   - plain inner\n",
            ),
            (
                "ordered-empty-outer-nested-task-bullet",
                "1. [ ]\n   - [x] nested\n",
            ),
            // ================================================================
            // DIMENSION: nesting depth (1, 2, 3 levels)
            // ================================================================

            // Depth 1 — covered above

            // Depth 2
            ("depth2-bullet-task", "- [ ] a\n  - [ ] b\n"),
            ("depth2-ordered-task", "1. [ ] a\n   1. [ ] b\n"),
            ("depth2-plain-nested-task", "- plain\n  - [ ] sub\n"),
            (
                "depth2-plain-outer-nested-task-with-sub",
                "- outer\n  - [ ] a\n    - [ ] b\n",
            ),
            (
                "depth2-plain-outer-nested-ordered-task-with-sub",
                "- outer\n  1. [ ] a\n     1. [ ] b\n",
            ),
            // Depth 3
            ("depth3-task", "- [ ] a\n  - [ ] b\n    - [x] c\n"),
            (
                "depth3-ordered-task",
                "1. [ ] a\n   1. [ ] b\n      1. [x] c\n",
            ),
            (
                "depth3-plain-outer",
                "- outer\n  - [ ] a\n    - [ ] b\n      - [x] c\n",
            ),
            // ================================================================
            // DIMENSION: interleaving (task-then-hoist, hoist-then-task, etc.)
            // ================================================================

            // Empty task first → hoist, then real task
            (
                "interleave-empty-hoist-then-task",
                "- [ ]\n  - plain inner\n- [x] after\n",
            ),
            // Real task, then empty+hoist, then real task
            (
                "interleave-task-hoist-task",
                "- [x] before\n- [ ]\n  - plain\n- [x] after\n",
            ),
            // Empty parent, two nested tasks
            (
                "interleave-empty-two-nested-tasks",
                "- [ ]\n  - [x] a\n  - [ ] b\n",
            ),
            // Real task, then empty + nested task
            (
                "interleave-real-then-empty-nested-task",
                "- [x] real\n- [ ]\n  - [ ] nested\n",
            ),
            // Task then empty-no-sub then task (empty with nothing)
            (
                "interleave-task-empty-nochild-task",
                "- [x] a\n- [ ]\n- [x] b\n",
            ),
            // All-empty tasks
            ("interleave-all-empty-tasks", "- [ ]\n- [ ]\n- [ ]\n"),
            // ================================================================
            // DIMENSION: container wrapping
            // ================================================================

            // Top-level (already covered above)

            // Inside blockquote
            ("blockquote-wraps-task-list", "> - [ ] in blockquote\n"),
            (
                "blockquote-wraps-nested-task",
                "> - [ ] outer\n>   - [x] nested\n",
            ),
            (
                "blockquote-wraps-empty-outer-nested-task",
                "> - [ ]\n>   - [x] inner\n",
            ),
            // Inside GFM alert panel
            ("panel-wraps-task-list", "> [!NOTE]\n> - [ ] in panel\n"),
            (
                "panel-wraps-nested-task",
                "> [!NOTE]\n> - [ ] outer\n>   - [x] nested\n",
            ),
            (
                "panel-wraps-ordered-task",
                "> [!WARNING]\n> 1. [ ] a\n> 2. [x] b\n",
            ),
            // Inside a plain listItem (plain outer wrapping task)
            (
                "list-item-wraps-task-list",
                "- outer item\n  - [ ] sub task\n",
            ),
            (
                "list-item-wraps-nested-task-list",
                "- outer item\n  - [ ] a\n    - [x] b\n",
            ),
            // ================================================================
            // EXACT TRIGGER INPUTS from prior adversarial passes
            // ================================================================

            // F-P11-001 exact triggers
            ("fp11-001-exact-1", "1. [ ]\n   1. [x] x\n"),
            ("fp11-001-exact-2", "1. [ ]\n   - plain inner\n"),
            ("fp11-001-exact-3", "1. [ ]\n   - [x] nested\n"),
            // F-PASS13-C1 exact triggers
            ("fpass13-c1-exact-1", "- outer\n  - [ ] a\n    - [ ] b\n"),
            ("fpass13-c1-exact-2", "- outer\n  1. [ ] a\n     1. [ ] b\n"),
            // Prior pass trigger inputs
            ("prior-loose-outer-plain-inner", "- [ ]\n\n  - inner\n"),
            (
                "prior-empty-then-plain-then-checked",
                "- [ ]\n  - plain\n- [x] after\n",
            ),
            // ================================================================
            // REGRESSION: plain ordered lists must not be reclassified
            // ================================================================
            ("regression-plain-ordered", "1. first\n2. second\n"),
            ("regression-nested-ordered-in-plain", "- item\n  1. sub\n"),
            ("regression-plain-bullet", "- a\n- b\n"),
            // ================================================================
            // CONTENT: task items with non-trivial inline content
            // ================================================================
            ("task-with-url", "- [ ] see https://example.com\n"),
            ("task-with-bold", "- [ ] **bold** item\n"),
            ("task-with-code", "- [ ] `code` item\n"),
            ("task-with-strikethrough", "- [ ] ~~done~~ item\n"),
        ];

        for (label, md) in inputs {
            // No-panic guarantee: markdown_to_adf must complete normally.
            let adf = markdown_to_adf(md).unwrap();

            // Structural validity: assert_valid_adf_structure panics on violation.
            assert_valid_adf_structure(&adf);

            // No empty list containers.
            assert_no_empty_list_content(&adf, label);

            // No-content-drop: every significant word in the input must appear
            // somewhere in the serialized ADF output.
            // (Skip inputs whose only content is task markers or empty.)
            let adf_str = serde_json::to_string(&adf).unwrap_or_default();
            for word in md.split_whitespace() {
                // Strip markdown syntax to get candidate content words.
                let stripped = word
                    .trim_start_matches("- ")
                    .trim_start_matches("1. ")
                    .trim_start_matches("[ ]")
                    .trim_start_matches("[x]")
                    .trim_start_matches("[!NOTE]")
                    .trim_start_matches("[!WARNING]")
                    .trim_start_matches('>')
                    .trim_start_matches('-')
                    .trim_start_matches("**")
                    .trim_end_matches("**")
                    .trim_start_matches("~~")
                    .trim_end_matches("~~")
                    .trim_matches('`')
                    .trim();
                // Only check words with ≥3 non-punctuation chars to avoid false
                // positives from markdown syntax tokens.
                let alpha_count = stripped.chars().filter(|c| c.is_alphabetic()).count();
                if alpha_count >= 3 && !stripped.starts_with("http") {
                    assert!(
                        adf_str.contains(stripped),
                        "[{label}] word {stripped:?} from input not found in ADF output.\n\
                         Input: {md:?}\n\
                         ADF: {adf}"
                    );
                }
            }
        }
    }

    // --- F6: property-based hardening for task-list markdown→ADF -------------
    //
    // F6 deliverable (#471). The F5 adversarial loop converged at 16 passes, but
    // the recurring defect class was *compositional* invalid-ADF: deep nesting,
    // ordered-list task markers, and empty-body items combined in ways that
    // hand-written example tests kept missing (e.g. `orderedList > taskItem`,
    // `bulletList > bulletList`, empty `taskList`, block-in-`taskItem`). A
    // generative property test is the right tool: it explores the composition
    // space that examples cannot enumerate.
    //
    // The strategy below builds RANDOM markdown biased toward task lists and
    // their compositions, then for each input asserts four invariants:
    //   (a) markdown_to_adf NEVER panics.
    //   (b) the produced ADF ALWAYS passes assert_valid_adf_structure (the F5
    //       recursive parent→child content-model validator) AND has no empty
    //       list/taskList content.
    //   (c) no temp underscore-prefixed keys leak (assert_no_underscore_keys).
    //   (d) adf_to_text(markdown_to_adf(input)) is total (never panics).
    //
    // Any failing input is auto-minimized by proptest; the minimized case is
    // then added as a deterministic regression unit test above and the
    // IMPLEMENTATION is fixed (via the shared helpers reclassify_as_task_list /
    // split_stray_blocks_end_result / normalize_*) — never by weakening the
    // property.

    use proptest::prelude::*;

    /// One generated markdown line item, before indentation/marker rendering.
    #[derive(Debug, Clone)]
    enum GenItem {
        /// Unchecked task item: `- [ ] body` / `1. [ ] body`.
        TaskUnchecked(String),
        /// Checked task item: `- [x] body` / `1. [x] body`.
        TaskChecked(String),
        /// Plain bullet/ordered item with no task marker.
        Plain(String),
        /// Empty-body task item (`- [ ]` with nothing after the marker) — the
        /// F-PASS3-C1 / F-471-M1 trigger class.
        EmptyTask(bool),
    }

    /// Marker style for a generated list: bullet (`-`) vs ordered (`1.`).
    #[derive(Debug, Clone, Copy)]
    enum GenMarker {
        Bullet,
        Ordered,
    }

    /// A wrapper applied to the whole generated block.
    #[derive(Debug, Clone, Copy)]
    enum GenWrap {
        None,
        Blockquote,
        PanelNote,
        PanelWarning,
    }

    /// A recursive tree of list items with optional nested sublists, plus a
    /// loose/tight flag and a per-item marker style.
    #[derive(Debug, Clone)]
    struct GenNode {
        item: GenItem,
        marker: GenMarker,
        /// Nested children, rendered at +2 indent. May be empty.
        children: Vec<GenNode>,
        /// Loose list → a trailing blank line after the item body.
        loose: bool,
    }

    /// Render a small inline-mark fragment for an item body. Kept short and
    /// deterministic-per-seed so the generated markdown stays human-plausible.
    fn render_body(words: &[u8]) -> String {
        if words.is_empty() {
            return "x".to_string();
        }
        let mut s = String::new();
        for (i, w) in words.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            match w % 5 {
                0 => s.push_str(&format!("**b{i}**")),
                1 => s.push_str(&format!("*i{i}*")),
                2 => s.push_str(&format!("`c{i}`")),
                3 => s.push_str(&format!("w{i}")),
                _ => s.push_str(&format!("[l{i}](https://e.x/{i})")),
            }
        }
        s
    }

    /// Recursively render a GenNode (and its children) into markdown lines at
    /// the given indentation depth.
    fn render_node(node: &GenNode, depth: usize, out: &mut String) {
        let indent = "  ".repeat(depth);
        let marker = match node.marker {
            GenMarker::Bullet => "-".to_string(),
            GenMarker::Ordered => "1.".to_string(),
        };
        match &node.item {
            GenItem::TaskUnchecked(body) => {
                out.push_str(&format!("{indent}{marker} [ ] {body}\n"));
            }
            GenItem::TaskChecked(body) => {
                out.push_str(&format!("{indent}{marker} [x] {body}\n"));
            }
            GenItem::Plain(body) => {
                out.push_str(&format!("{indent}{marker} {body}\n"));
            }
            GenItem::EmptyTask(checked) => {
                let box_ = if *checked { "[x]" } else { "[ ]" };
                // Empty body: marker + checkbox + nothing.
                out.push_str(&format!("{indent}{marker} {box_}\n"));
            }
        }
        for child in &node.children {
            render_node(child, depth + 1, out);
        }
        if node.loose {
            out.push('\n');
        }
    }

    /// Apply a block-level wrapper (blockquote / GFM-alert panel) to a rendered
    /// markdown block by prefixing each line with `> `.
    fn apply_wrap(wrap: GenWrap, body: &str) -> String {
        let prefixed = || {
            body.lines()
                .map(|l| {
                    if l.is_empty() {
                        ">".to_string()
                    } else {
                        format!("> {l}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        match wrap {
            GenWrap::None => body.to_string(),
            GenWrap::Blockquote => format!("{}\n", prefixed()),
            GenWrap::PanelNote => format!("> [!NOTE]\n{}\n", prefixed()),
            GenWrap::PanelWarning => format!("> [!WARNING]\n{}\n", prefixed()),
        }
    }

    /// proptest strategy for a single GenItem.
    fn gen_item() -> impl Strategy<Value = GenItem> {
        prop_oneof![
            // Bias toward task items (the feature under test) but keep plain and
            // empty-body items well represented because they are the boundary
            // classes that produced the recurring invalid-ADF defects.
            3 => proptest::collection::vec(any::<u8>(), 0..4)
                .prop_map(|w| GenItem::TaskUnchecked(render_body(&w))),
            3 => proptest::collection::vec(any::<u8>(), 0..4)
                .prop_map(|w| GenItem::TaskChecked(render_body(&w))),
            2 => proptest::collection::vec(any::<u8>(), 0..4)
                .prop_map(|w| GenItem::Plain(render_body(&w))),
            2 => any::<bool>().prop_map(GenItem::EmptyTask),
        ]
    }

    fn gen_marker() -> impl Strategy<Value = GenMarker> {
        prop_oneof![Just(GenMarker::Bullet), Just(GenMarker::Ordered)]
    }

    /// Recursive proptest strategy for a GenNode tree, nesting up to depth 5.
    fn gen_node() -> impl Strategy<Value = GenNode> {
        let leaf =
            (gen_item(), gen_marker(), any::<bool>()).prop_map(|(item, marker, loose)| GenNode {
                item,
                marker,
                children: vec![],
                loose,
            });
        // depth 5, up to 3 children per level, up to ~24 total nodes.
        leaf.prop_recursive(5, 24, 3, |inner| {
            (
                gen_item(),
                gen_marker(),
                proptest::collection::vec(inner, 0..3),
                any::<bool>(),
            )
                .prop_map(|(item, marker, children, loose)| GenNode {
                    item,
                    marker,
                    children,
                    loose,
                })
        })
    }

    fn gen_wrap() -> impl Strategy<Value = GenWrap> {
        prop_oneof![
            3 => Just(GenWrap::None),
            1 => Just(GenWrap::Blockquote),
            1 => Just(GenWrap::PanelNote),
            1 => Just(GenWrap::PanelWarning),
        ]
    }

    /// Top-level strategy: 1-4 sibling top-level nodes plus an optional wrapper.
    fn gen_markdown() -> impl Strategy<Value = String> {
        (proptest::collection::vec(gen_node(), 1..4), gen_wrap()).prop_map(|(nodes, wrap)| {
            let mut body = String::new();
            for node in &nodes {
                render_node(node, 0, &mut body);
            }
            apply_wrap(wrap, &body)
        })
    }

    proptest! {
        // 512 cases keeps CI bounded (~a few seconds) while exploring the deep
        // nesting / ordered / empty-body composition space that example tests
        // missed. Bump locally for a longer soak.
        #![proptest_config(ProptestConfig::with_cases(512))]

        #[test]
        fn prop_task_list_markdown_always_valid_adf(md in gen_markdown()) {
            // (a) markdown_to_adf is total (never panics). If it panicked,
            // proptest would catch it here and minimize.
            let adf = markdown_to_adf(&md).unwrap();

            // (b) structural validity: parent→child content-model legality for
            // every node touched by the task-list feature (bulletList/orderedList
            // children are listItem; taskList tuple-lead + taskItem/taskList only;
            // taskItem is inline-only; listItem/blockquote/panel allowlists).
            assert_valid_adf_structure(&adf);
            // ...and no empty list/taskList content (minItems:1).
            assert_no_empty_list_content(&adf, "proptest");

            // (c) no temp underscore-prefixed keys leak into the output (a leak
            // would cause Jira HTTP 400 via additionalProperties:false).
            assert_no_underscore_keys(&adf, "root");

            // (d) the reverse render is total too.
            let _ = adf_to_text(&adf).unwrap();
        }
    }

    // --- VP-571-001: code-mark exclusivity universal invariant (BC-7.2.015) ----
    //
    // Property: for every ADF text node anywhere in the tree whose `marks` array
    // contains `{"type":"code"}`, that array contains NO mark type outside the
    // allow-set `{"code", "link", "annotation"}`.  This is the BC-7.2.015
    // positive invariant expressed as a universal quantifier over generated
    // markdown inputs covering all 9 container contexts and all inline
    // code-composition templates (VP-571-001 spec: verification-delta-571.md).
    //
    // Generator scope authority (F2 R9 locked): full generator required —
    // 11 inline templates × 9 container wrapper kinds, wrapper depth ≤ 3,
    // GFM alert wrapper OUTERMOST-ONLY (Footnote A).  No MVP subset authorized.

    /// Inline code-composition template variant for VP-571-001 generator.
    #[derive(Debug, Clone)]
    enum MarkCompositionTemplate {
        Plain(String),                                // `{body}`
        Strong(String),                               // **`{body}`**
        Em(String),                                   // _`{body}`_
        Strike(String),                               // ~~`{body}`~~
        SupCode(String),                              // ^`{body}`^  (subsup sup — EC-4 primary)
        SubCode(String),                              // ~`{body}`~  (subsup sub — EC-4 variant)
        LinkCode { body: String, seg: String },       // [`{body}`](https://x/{seg})
        MixedStrong(String),                          // **a `{body}` c**
        MixedEm(String),                              // _a `{body}` c_
        NestedEmStrong(String),                       // **_`{body}`_**
        NestedLinkBold { body: String, seg: String }, // [**`{body}`**](https://x/{seg})
    }

    impl MarkCompositionTemplate {
        fn render(&self) -> String {
            match self {
                MarkCompositionTemplate::Plain(b) => format!("`{b}`"),
                MarkCompositionTemplate::Strong(b) => format!("**`{b}`**"),
                MarkCompositionTemplate::Em(b) => format!("_`{b}`_"),
                MarkCompositionTemplate::Strike(b) => format!("~~`{b}`~~"),
                MarkCompositionTemplate::SupCode(b) => format!("^`{b}`^"),
                MarkCompositionTemplate::SubCode(b) => format!("~`{b}`~"),
                MarkCompositionTemplate::LinkCode { body, seg } => {
                    format!("[`{body}`](https://x/{seg})")
                }
                MarkCompositionTemplate::MixedStrong(b) => format!("**a `{b}` c**"),
                MarkCompositionTemplate::MixedEm(b) => format!("_a `{b}` c_"),
                MarkCompositionTemplate::NestedEmStrong(b) => format!("**_`{b}`_**"),
                MarkCompositionTemplate::NestedLinkBold { body, seg } => {
                    format!("[**`{body}`**](https://x/{seg})")
                }
            }
        }
    }

    /// Container wrapper variant for VP-571-001 generator.
    /// Alert is excluded from inner levels (outermost-only constraint — Footnote A).
    #[derive(Debug, Clone, Copy)]
    enum WrapKind571 {
        None,
        Blockquote,
        UnorderedList,
        OrderedList,
        TaskListUnchecked,
        TaskListChecked,
        Heading,
        TableCell,
        FootnoteDef,
        Alert, // outermost-only (Footnote A)
    }

    /// Apply a single container wrapper to a (possibly multi-line) content string.
    fn wrap_mk571(kind: WrapKind571, content: &str) -> String {
        match kind {
            WrapKind571::None => content.to_owned(),
            WrapKind571::Blockquote => content
                .lines()
                .map(|l| {
                    if l.is_empty() {
                        ">".to_owned()
                    } else {
                        format!("> {l}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
            WrapKind571::UnorderedList => format!("- {content}"),
            WrapKind571::OrderedList => format!("1. {content}"),
            WrapKind571::TaskListUnchecked => format!("- [ ] {content}"),
            WrapKind571::TaskListChecked => format!("- [x] {content}"),
            WrapKind571::Heading => format!("## {content}"),
            WrapKind571::TableCell => {
                // 2-column, 1-row header + 1-row body (VP-571-001 Footnote B).
                format!("| {content} | plain |\n|---|---|\n| plain | {content} |")
            }
            WrapKind571::FootnoteDef => {
                // Reference + definition body (VP-571-001 Footnote C).
                format!("Body.[^fn]\n\n[^fn]: {content}")
            }
            WrapKind571::Alert => {
                // GFM alert — outermost-only (VP-571-001 Footnote A).
                // Prefix each content line with `> ` and prepend `> [!NOTE]`.
                let prefixed = content
                    .lines()
                    .map(|l| {
                        if l.is_empty() {
                            ">".to_owned()
                        } else {
                            format!("> {l}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("> [!NOTE]\n{prefixed}\n")
            }
        }
    }

    /// Generate a short lowercase alphabetic body string for inline templates.
    fn gen_mk571_body() -> impl Strategy<Value = String> {
        proptest::string::string_regex("[a-z]{1,4}").unwrap()
    }

    /// Generate a short URL path segment.
    fn gen_mk571_seg() -> impl Strategy<Value = String> {
        proptest::string::string_regex("[a-z]{1,4}").unwrap()
    }

    /// Strategy for all 11 inline code-composition templates (VP-571-001).
    /// Equal weights — each ~9% ≫ the ~5% floor.
    fn gen_mark_composition_template() -> impl Strategy<Value = MarkCompositionTemplate> {
        (gen_mk571_body(), gen_mk571_seg()).prop_flat_map(|(body, seg)| {
            prop_oneof![
                Just(MarkCompositionTemplate::Plain(body.clone())),
                Just(MarkCompositionTemplate::Strong(body.clone())),
                Just(MarkCompositionTemplate::Em(body.clone())),
                Just(MarkCompositionTemplate::Strike(body.clone())),
                Just(MarkCompositionTemplate::SupCode(body.clone())),
                Just(MarkCompositionTemplate::SubCode(body.clone())),
                Just(MarkCompositionTemplate::LinkCode {
                    body: body.clone(),
                    seg: seg.clone()
                }),
                Just(MarkCompositionTemplate::MixedStrong(body.clone())),
                Just(MarkCompositionTemplate::MixedEm(body.clone())),
                Just(MarkCompositionTemplate::NestedEmStrong(body.clone())),
                Just(MarkCompositionTemplate::NestedLinkBold {
                    body: body.clone(),
                    seg: seg.clone()
                }),
            ]
        })
    }

    /// Inner container wrapper strategy: 9 variants (excluding alert).
    /// Alert is excluded here — it may only appear as the outermost wrap.
    fn gen_inner_wrap571() -> impl Strategy<Value = WrapKind571> {
        prop_oneof![
            Just(WrapKind571::None),
            Just(WrapKind571::Blockquote),
            Just(WrapKind571::UnorderedList),
            Just(WrapKind571::OrderedList),
            Just(WrapKind571::TaskListUnchecked),
            Just(WrapKind571::TaskListChecked),
            Just(WrapKind571::Heading),
            Just(WrapKind571::TableCell),
            Just(WrapKind571::FootnoteDef),
        ]
    }

    /// Outermost wrapper strategy: 10 variants (inner 9 + Alert).
    /// Alert is included here because it is only valid as the outermost wrapper.
    fn gen_outer_wrap571() -> impl Strategy<Value = WrapKind571> {
        prop_oneof![
            Just(WrapKind571::None),
            Just(WrapKind571::Blockquote),
            Just(WrapKind571::UnorderedList),
            Just(WrapKind571::OrderedList),
            Just(WrapKind571::TaskListUnchecked),
            Just(WrapKind571::TaskListChecked),
            Just(WrapKind571::Heading),
            Just(WrapKind571::TableCell),
            Just(WrapKind571::FootnoteDef),
            Just(WrapKind571::Alert),
        ]
    }

    /// Top-level generator for VP-571-001: inline template + ≤3 total wrapper
    /// layers (0–2 inner, 1 outermost).  Alert is restricted to the outermost
    /// layer (Footnote A).  All 9 container contexts are reachable; wrapper
    /// depth budget ≤ 3 keeps ADF tree depth well below MAX_ADF_DEPTH = 256.
    fn gen_mark_composition_markdown() -> impl Strategy<Value = String> {
        (
            gen_mark_composition_template(),
            // 0–2 inner wrappers (no alert — Footnote A)
            proptest::collection::vec(gen_inner_wrap571(), 0..=2),
            // 1 outermost wrapper (alert permitted here only)
            gen_outer_wrap571(),
        )
            .prop_map(|(tmpl, inner_wraps, outer)| {
                let mut content = tmpl.render();
                // Apply inner wrappers (each layers inside the previous).
                for wk in inner_wraps {
                    content = wrap_mk571(wk, &content);
                }
                // Apply the outermost wrapper last.
                wrap_mk571(outer, &content)
            })
    }

    proptest! {
        // Default ~256 cases is sufficient for this universal invariant over
        // a small-alphabet, bounded-depth generator.  Cap to 128 with
        // `ProptestConfig { cases: 128, .. }` only if CI flake pressure appears
        // (VP-571-001 §"Cases required from proptest").
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// VP-571-001: for every markdown input in `gen_mark_composition_markdown()`,
        /// `markdown_to_adf` produces an ADF document where no text node with a
        /// `code` mark carries any typographic mark (strong/em/strike/subsup/…).
        ///
        /// BC-7.2.015 positive invariant expressed as a universal quantifier.
        /// Grounded in `src/adf.rs::push_code` (sole emit site for code marks).
        #[test]
        fn prop_bc_7_2_015_no_code_marked_text_node_carries_typographic_marks(
            md in gen_mark_composition_markdown()
        ) {
            // markdown_to_adf must not panic or exceed depth limit on generated
            // inputs (wrapper budget ≤ 3 keeps ADF depth well below 256).
            let adf = markdown_to_adf(&md).expect("no depth error at this bound");
            // Universal invariant: every code-marked text node is typographic-mark-free.
            assert_code_mark_exclusivity(&adf);
        }
    }

    // --- F6: property-based hardening for block-HTML markdown→ADF (#492) -------
    //
    // F6 deliverable (#492, "Algorithm B", BC-7.2.011). The F5 adversarial loop
    // verified the block-HTML hardBreak handler by hand; this suite formalizes
    // those invariants generatively. The handler's job: block-HTML interior
    // newlines become ADF `hardBreak` nodes so that NO ADF `text` node ever
    // contains a raw `\n`/`\r` (Jira rejects those with HTTP 400).
    //
    // Two generators feed the properties:
    //   * gen_arbitrary_string — fully arbitrary text (the file-wide invariant
    //     INV-1/INV-2/INV-4 must hold for ALL markdown, not just block HTML).
    //   * gen_block_html — `<tag>`-wrapped multi-line bodies with interleaved
    //     `\n`, `\r\n`, lone `\r`, blank lines, leading/trailing whitespace,
    //     multibyte UTF-8, and bare URLs (the Algorithm B trigger surface).
    //
    // Invariants asserted (per the adversarial-review hand checks):
    //   INV-1  no `text` node anywhere contains `\n` or `\r` (the bug class).
    //   INV-2  no `paragraph` node has an empty `content` array.
    //   INV-3  no manufactured block-HTML paragraph begins/ends with hardBreak.
    //   INV-4  markdown_to_adf never panics (proptest surfaces panics).
    //   INV-5  for block-HTML inputs, adf_to_text(markdown_to_adf(x)) is
    //          line-structure-lossless per BC-7.2.011 (LF-normalized form).
    //
    // A falsifying input is a CRITICAL finding: fix the IMPLEMENTATION, never
    // weaken the property.

    /// Recursively walk an ADF tree, invoking `f` on every node (object) along
    /// with a flag indicating whether the node is inside a `codeBlock` subtree.
    /// `codeBlock` content is preformatted: an embedded `\n` is *valid* ADF there
    /// (multi-line code is stored as a single text node with literal newlines),
    /// so callers that enforce the paragraph "no raw `\n`" rule must exempt it.
    fn for_each_adf_node_ctx(
        node: &serde_json::Value,
        in_code_block: bool,
        f: &mut impl FnMut(&serde_json::Value, bool),
    ) {
        match node {
            serde_json::Value::Object(_) => {
                f(node, in_code_block);
                let is_code = node.get("type").and_then(|t| t.as_str()) == Some("codeBlock");
                if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                    for child in content {
                        for_each_adf_node_ctx(child, in_code_block || is_code, f);
                    }
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    for_each_adf_node_ctx(item, in_code_block, f);
                }
            }
            _ => {}
        }
    }

    /// Recursively walk an ADF tree, invoking `f` on every node (object).
    fn for_each_adf_node(node: &serde_json::Value, f: &mut impl FnMut(&serde_json::Value)) {
        for_each_adf_node_ctx(node, false, &mut |n, _| f(n));
    }

    /// INV-1: No `text` node OUTSIDE a `codeBlock` may carry a raw `\n` (Jira
    /// rejects raw newlines in paragraph/inline text — HTTP 400). This is the
    /// core invariant the #492 Algorithm B block-HTML handler governs, and the
    /// #522 context-aware `push_text`/`push_code` normalization upholds.
    ///
    /// Additionally, no `text` node anywhere in the tree may carry a raw `\r`:
    /// the #492 Algorithm B handler (block-HTML) and the #522 `push_text`/
    /// `push_code` normalization (generic parser path) both guarantee CR-free
    /// output. The `\r` check is unconditional — as of #522 all code paths
    /// normalize CR before constructing any text node (BC-7.2.011 EC-11,
    /// F5 Pass-1).
    ///
    /// SCOPE NOTE (codeBlock `\n` exemption): `codeBlock` content is preformatted —
    /// multi-line code is legally stored as a single `text` node with literal
    /// `\n` newlines, which Jira accepts — so the `\n` clause exempts codeBlock.
    /// The `\r` clause applies everywhere (no exemption).
    fn assert_no_raw_newline_in_text_nodes(adf: &serde_json::Value, input: &str) {
        for_each_adf_node_ctx(adf, false, &mut |node, in_code_block| {
            if node.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
                    // `\n` clause: forbidden in paragraph/inline text; codeBlock
                    // interiors are exempt (preformatted, embedded \n is valid).
                    if !in_code_block {
                        assert!(
                            !text.contains('\n'),
                            "INV-1 VIOLATED: non-codeBlock text node contains raw \\n.\n\
                             input={input:?}\ntext node={text:?}\nfull adf={adf}"
                        );
                    }
                    // `\r` clause: unconditional — all code paths normalize CR before
                    // constructing any text node (#492 Algorithm B + #522 push_text).
                    assert!(
                        !text.contains('\r'),
                        "INV-1 VIOLATED: text node contains raw \\r \
                         (CR normalization failed).\n\
                         input={input:?}\ntext node={text:?}\nfull adf={adf}"
                    );
                }
            }
        });
    }

    /// INV-2 (file-wide form): every `paragraph` node must carry a `content` key
    /// whose value is an array. Per the ADF spec (Perplexity-verified 2026-06),
    /// an explicit empty `content: []` is VALID and accepted by Jira — it is the
    /// canonical empty paragraph (e.g. the placeholder inside an empty
    /// `listItem`, which `is_empty_block_container` deliberately preserves). The
    /// real Jira-400 hazard is a paragraph with NO `content` key at all
    /// (`{"type":"paragraph"}`), which violates the schema. This invariant
    /// therefore asserts the `content` key is present-and-an-array, not that it
    /// is non-empty.
    fn assert_paragraph_has_content_key(adf: &serde_json::Value, input: &str) {
        for_each_adf_node(adf, &mut |node| {
            if node.get("type").and_then(|t| t.as_str()) == Some("paragraph") {
                assert!(
                    node.get("content").and_then(|c| c.as_array()).is_some(),
                    "INV-2 VIOLATED: paragraph node missing a `content` array key \
                     (keyless paragraph is invalid ADF → Jira 400).\n\
                     input={input:?}\nfull adf={adf}"
                );
            }
        });
    }

    /// INV-2 (block-HTML-strict form): the #492 Algorithm B handler NEVER emits an
    /// empty manufactured paragraph — step 6 early-returns `EndResult::Empty` when
    /// the trimmed content array is empty, so no paragraph is produced at all.
    /// For block-HTML-shaped inputs we therefore assert the stronger property the
    /// delta actually upholds: no `paragraph` anywhere has an empty `content`
    /// array. (This holds for the block-HTML generator because it cannot produce
    /// the empty-listItem-placeholder paragraph that the arbitrary generator can.)
    fn assert_no_empty_paragraph_strict(adf: &serde_json::Value, input: &str) {
        for_each_adf_node(adf, &mut |node| {
            if node.get("type").and_then(|t| t.as_str()) == Some("paragraph") {
                if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                    assert!(
                        !content.is_empty(),
                        "INV-2 VIOLATED (strict): block-HTML produced a paragraph \
                         with an empty content array.\ninput={input:?}\nfull adf={adf}"
                    );
                }
            }
        });
    }

    /// INV-3: assert no `paragraph` begins or ends with a `hardBreak` node.
    ///
    /// SCOPE NOTE: this is the #492 Algorithm B step-5b contract (trim leading/
    /// trailing manufactured hardBreaks). It is asserted only over BLOCK-HTML
    /// inputs, because a NORMAL markdown paragraph can *legitimately* begin or end
    /// with a hardBreak via a backslash-hard-break at a line edge (a CommonMark
    /// trailing backslash before a line ending maps to a hardBreak), which is
    /// valid ADF and outside #492's scope. Applying this to arbitrary markdown
    /// would false-positive on that construct.
    fn assert_no_paragraph_edge_hardbreak(adf: &serde_json::Value, input: &str) {
        for_each_adf_node(adf, &mut |node| {
            if node.get("type").and_then(|t| t.as_str()) == Some("paragraph") {
                if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                    if let Some(first) = content.first() {
                        assert_ne!(
                            first.get("type").and_then(|t| t.as_str()),
                            Some("hardBreak"),
                            "INV-3 VIOLATED: paragraph begins with hardBreak.\n\
                             input={input:?}\nfull adf={adf}"
                        );
                    }
                    if let Some(last) = content.last() {
                        assert_ne!(
                            last.get("type").and_then(|t| t.as_str()),
                            Some("hardBreak"),
                            "INV-3 VIOLATED: paragraph ends with hardBreak.\n\
                             input={input:?}\nfull adf={adf}"
                        );
                    }
                }
            }
        });
    }

    /// LF-normalize and strip per-line trailing whitespace + trailing blank
    /// lines. This is the canonical form against which INV-5's line-structure
    /// losslessness is measured: BC-7.2.011 documents exactly five non-byte-
    /// identical round-trip cases (CRLF→LF, leading-newline hardBreak trim,
    /// trailing-newline strip, trailing-non-newline-whitespace strip, and the
    /// #473 bare-URL `[url](url)` rewrite). Cases 1-4 are all normalized away by
    /// this function; case 5 is excluded by construction in `gen_block_html`
    /// (bodies never contain a bare `http(s)://` URL), so the comparison is exact.
    fn lf_canonical(s: &str) -> String {
        let lf = s.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines: Vec<&str> = lf
            .lines()
            .map(|l| l.trim_end_matches([' ', '\t']))
            .collect();
        // Strip trailing blank lines (case 3: trailing-newline strip).
        while matches!(lines.last(), Some(&"")) {
            lines.pop();
        }
        // Strip leading blank lines (case 2: leading-newline hardBreak trim).
        while matches!(lines.first(), Some(&"")) {
            lines.remove(0);
        }
        lines.join("\n")
    }

    /// A generated block-HTML body line: deliberately exercises the trim/normalize
    /// surface without introducing constructs that would change the line count
    /// (no bare http(s):// URLs → no #473 autolink rewrite; the strings stay
    /// literal text inside the manufactured paragraph).
    fn gen_html_body_line() -> impl Strategy<Value = String> {
        prop_oneof![
            // Plain ASCII run with angle-bracket-ish HTML shape.
            "[a-zA-Z0-9 .,_/=\"'-]{0,24}".prop_map(|s| format!("<span>{s}</span>")),
            // Leading/trailing whitespace (interior to the body — the line still
            // carries content so it is not a blank line).
            "[a-zA-Z0-9]{1,8}".prop_map(|s| format!("  {s}  ")),
            // Multibyte UTF-8 (emoji, accents, CJK) to prove byte-vs-char safety.
            Just("café — 日本語 🚀 <b>×</b>".to_string()),
            Just("naïve façade Ω≈ç√∫".to_string()),
            // A www-prefixed / scheme-less host: stays literal (out of #473 scope),
            // so it does NOT trigger the bare-URL rewrite and the line count holds.
            Just("see www.example.com/path for info".to_string()),
            // Blank line (interior blank → a pure hardBreak boundary, no text).
            Just(String::new()),
            // Tag-only lines.
            Just("<div>".to_string()),
            Just("</div>".to_string()),
            Just("<br/>".to_string()),
        ]
    }

    /// Build a block-HTML markdown input from generated body lines joined by a
    /// generated mix of line endings, wrapped so pulldown-cmark routes it through
    /// the HtmlBlock path. The joiner set covers LF, CRLF, and lone CR; we record
    /// nothing about the joiners because INV-5 measures the LF-canonical form.
    ///
    /// NOTE: a single interior BLANK line terminates a CommonMark type-6 HTML
    /// block, which would split the run into two ADF paragraphs. INV-5 (line-
    /// structure losslessness for ONE manufactured paragraph) is only meaningful
    /// for a single uninterrupted block, so the INV-5 property restricts itself
    /// to no-interior-blank inputs; INV-1..4 still run on the full generator.
    fn gen_block_html() -> impl Strategy<Value = String> {
        (
            proptest::collection::vec(gen_html_body_line(), 1..6),
            // Per-boundary line-ending choice: 0=LF, 1=CRLF, 2=lone CR.
            proptest::collection::vec(0u8..3, 0..6),
            // Optional leading blank line and trailing newline run.
            any::<bool>(),
            0usize..3,
        )
            .prop_map(|(lines, endings, leading_blank, trailing_nls)| {
                let mut s = String::new();
                if leading_blank {
                    s.push('\n');
                }
                // Always open with a block-level tag on its own line so pulldown
                // recognizes a type-6 HTML block regardless of the first body line.
                s.push_str("<div>\n");
                for (i, line) in lines.iter().enumerate() {
                    s.push_str(line);
                    if i + 1 < lines.len() {
                        let e = endings.get(i).copied().unwrap_or(0);
                        match e % 3 {
                            0 => s.push('\n'),
                            1 => s.push_str("\r\n"),
                            _ => s.push('\r'),
                        }
                    }
                }
                s.push_str("\n</div>");
                for _ in 0..trailing_nls {
                    s.push('\n');
                }
                s
            })
    }

    /// True if the LF-normalized input contains an interior blank line (which
    /// terminates a CommonMark HTML block and splits it across paragraphs).
    fn has_interior_blank_line(input: &str) -> bool {
        let lf = input.replace("\r\n", "\n").replace('\r', "\n");
        let trimmed = lf.trim_matches('\n');
        trimmed.contains("\n\n")
    }

    /// Block-HTML generator for INV-5: identical surface to `gen_block_html` but
    /// guarantees NO interior blank line (and no leading blank), so the result is
    /// always a SINGLE uninterrupted CommonMark type-6 HTML block. This is built
    /// by *construction* (blank body lines are mapped to a non-empty placeholder)
    /// rather than by `prop_assume!` rejection — at high case counts a reject-based
    /// filter blows past proptest's global-reject cap. Trailing newlines are still
    /// exercised (they don't split the block; they're stripped by `lf_canonical`).
    fn gen_block_html_no_blank() -> impl Strategy<Value = String> {
        (
            proptest::collection::vec(gen_html_body_line(), 1..6),
            proptest::collection::vec(0u8..3, 0..6),
            0usize..3,
        )
            .prop_map(|(lines, endings, trailing_nls)| {
                let mut s = String::from("<div>\n");
                for (i, line) in lines.iter().enumerate() {
                    // Map any blank body line to a non-empty placeholder so the
                    // block never contains an interior blank line.
                    let line = if line.is_empty() {
                        "<hr/>"
                    } else {
                        line.as_str()
                    };
                    s.push_str(line);
                    if i + 1 < lines.len() {
                        let e = endings.get(i).copied().unwrap_or(0);
                        match e % 3 {
                            0 => s.push('\n'),
                            1 => s.push_str("\r\n"),
                            _ => s.push('\r'),
                        }
                    }
                }
                s.push_str("\n</div>");
                for _ in 0..trailing_nls {
                    s.push('\n');
                }
                s
            })
    }

    proptest! {
        // 2048 cases/property — matches the order of magnitude used by the
        // Windows F6 path-fallback suite. Keeps CI in the low-seconds range while
        // densely sampling the newline/whitespace/UTF-8 composition space.
        #![proptest_config(ProptestConfig::with_cases(2048))]

        /// INV-1/2/3/4 over FULLY ARBITRARY markdown. The file-wide invariants
        /// must hold for every possible string, not just block HTML.
        ///
        /// Strategy: explicit charset `[\r\n\t a-zA-Z0-9]{0,64}` so that `\r`
        /// and `\n` (the INV-1 control chars) are sampled on every run.  The
        /// former `".*"` strategy silently excluded `\n` because Rust regex `.`
        /// does not match newlines by default (F5 finding F-1).
        #[test]
        fn prop_492_arbitrary_string_holds_core_invariants(
            input in "[\\r\\n\\t a-zA-Z0-9]{0,64}",
        ) {
            // INV-4: markdown_to_adf must not panic (proptest surfaces a panic
            // here and minimizes the input automatically).
            let adf = markdown_to_adf(&input).unwrap();
            // INV-1: no raw `\n` or `\r` in any non-codeBlock text node, and no
            // raw `\r` anywhere. Fixed in #522: push_text/push_code normalize lone
            // \r on the generic parser path (BC-7.2.011 EC-11 / INV-push-text-cr).
            assert_no_raw_newline_in_text_nodes(&adf, &input);
            // INV-2 (file-wide): every paragraph carries a `content` array key
            // (empty `[]` is valid ADF; a keyless paragraph is the real hazard).
            assert_paragraph_has_content_key(&adf, &input);
            // INV-3 is NOT asserted here: a normal markdown paragraph may
            // legitimately edge with a hardBreak (backslash hard-break at a line
            // boundary). INV-3 is a block-HTML-delta contract — see that property.
            // INV-4 (reverse): adf_to_text must also be total.
            let _ = adf_to_text(&adf).unwrap();
        }

        /// INV-1/2/3/4 over BLOCK-HTML-SHAPED inputs (the Algorithm B surface):
        /// `<div>`-wrapped multi-line bodies with interleaved LF/CRLF/lone-CR,
        /// blank lines, leading/trailing whitespace, and multibyte UTF-8.
        #[test]
        fn prop_492_block_html_holds_core_invariants(input in gen_block_html()) {
            let adf = markdown_to_adf(&input).unwrap();     // INV-4
            // INV-1: block-HTML text nodes must be free of both \n (non-codeBlock)
            // and \r (all nodes). Algorithm B step-3 normalizes \r; push_text
            // context-aware normalization (#522) covers the generic path.
            assert_no_raw_newline_in_text_nodes(&adf, &input);  // INV-1
            assert_paragraph_has_content_key(&adf, &input);     // INV-2 (file-wide)
            assert_no_empty_paragraph_strict(&adf, &input);     // INV-2 (delta-strict)
            assert_no_paragraph_edge_hardbreak(&adf, &input);   // INV-3
            let _ = adf_to_text(&adf).unwrap();             // INV-4 reverse
        }

        /// INV-5: for a SINGLE uninterrupted block-HTML run (no interior blank
        /// line), the ADF→text round trip is line-structure-lossless in the
        /// LF-canonical form (CRLF/CR→LF, per-line trailing-whitespace stripped,
        /// trailing blank lines removed). The five documented non-byte-identical
        /// cases are all normalized away by `lf_canonical` (cases 1-4) or excluded
        /// by construction (case 5: bodies carry no bare http(s):// URL).
        #[test]
        fn prop_492_block_html_round_trip_line_structure_lossless(
            input in gen_block_html_no_blank()
        ) {
            // The generator guarantees no interior blank line by construction;
            // this assertion documents/enforces that contract (never rejects).
            prop_assert!(!has_interior_blank_line(&input));
            let adf = markdown_to_adf(&input).unwrap();
            let rendered = adf_to_text(&adf).unwrap();
            prop_assert_eq!(
                lf_canonical(&rendered),
                lf_canonical(&input),
                "INV-5 VIOLATED: round-trip not line-structure-lossless.\n\
                 input={:?}\nrendered={:?}\nadf={}",
                input,
                rendered,
                adf
            );
        }
    }

    /// Collect every `text` node's string in the tree (DFS order).
    fn collect_all_text_nodes(adf: &serde_json::Value) -> Vec<String> {
        let mut out = Vec::new();
        for_each_adf_node(adf, &mut |node| {
            if node.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(t) = node.get("text").and_then(|t| t.as_str()) {
                    out.push(t.to_string());
                }
            }
        });
        out
    }

    /// BC-7.2.011 EC-11 (INV-push-text-cr) — fixed in #522 (F5 Pass-1 hardening).
    ///
    /// `AdfBuilder::push_text` must normalize CR before constructing any ADF text
    /// node. The correct substitution is context-dependent (empirically confirmed):
    ///
    /// - **Inside `codeBlock`:** lone `\r`→`\n` (codeBlock text nodes allow `\n`
    ///   for multi-line preformatted content).
    /// - **Outside `codeBlock` (heading, paragraph, inline text):** lone `\r`→ space
    ///   (mirrors `SoftBreak → push_text(" ")`; raw `\n` is forbidden in non-codeBlock
    ///   text nodes — INV-1 — so converting `\r`→`\n` would trade one violation for
    ///   another). pulldown-cmark emits `Text("x\ry")` for `# x\ry` (heading with lone
    ///   CR), so the space substitution is the minimal correct fix.
    ///
    /// Red Gate: FAILS before the fix (push_text does not normalize `\r`).
    /// Green Gate: passes after push_text gains context-aware CR-normalization.
    #[test]
    fn test_push_text_normalizes_lone_cr_in_heading_and_code_block() {
        // indented codeBlock: lone \r in input → \n in codeBlock text (allowed).
        let cb = markdown_to_adf("\ta\r").unwrap();
        let cb_texts = collect_all_text_nodes(&cb);
        assert!(
            cb_texts.iter().all(|t| !t.contains('\r')),
            "push_text must normalize lone \\r in indented codeBlock; \
             text nodes must contain no raw \\r (BC-7.2.011 EC-11)"
        );
        // AC-001: the codeBlock text node value must be EXACTLY "a\n" — the lone \r
        // is replaced by \n (codeBlock context allows \n), and no other characters
        // are present. Pins the exact postcondition from BC-7.2.011 EC-11.
        assert!(
            cb_texts.iter().any(|t| t == "a\n"),
            "push_text must produce exactly \"a\\n\" as the codeBlock text node \
             for input \"\\ta\\r\"; got: {:?} (BC-7.2.011 EC-11 AC-001)",
            cb_texts
        );
        // heading: pulldown emits Text("x\ry"); push_text must convert \r → space,
        // NOT \n (which would violate INV-1 — raw \n forbidden in heading text nodes).
        let h = markdown_to_adf("# x\ry").unwrap();
        let h_texts = collect_all_text_nodes(&h);
        assert!(
            h_texts.iter().all(|t| !t.contains('\r')),
            "push_text must normalize lone \\r in heading; \
             text nodes must contain no raw \\r (BC-7.2.011 EC-11)"
        );
        assert!(
            h_texts.iter().all(|t| !t.contains('\n')),
            "push_text must NOT introduce raw \\n in heading text nodes (INV-1); \
             \\r must become space, not \\n. got: {:?} (BC-7.2.011 EC-11)",
            h_texts
        );
        // The heading text must contain the space-substituted form "x y".
        assert!(
            h_texts.iter().any(|t| t == "x y"),
            "push_text must convert lone \\r to space in heading; \
             expected text node \"x y\" but got: {:?} (BC-7.2.011 EC-11)",
            h_texts
        );
    }

    /// BC-7.2.011 EC-11 — lone `\r` in a fenced codeBlock content reaches `push_text`
    /// via `Event::Text` and must be normalized to `\n` (codeBlock context allows `\n`).
    ///
    /// pulldown-cmark passes fenced codeBlock body text verbatim, including lone `\r`,
    /// unlike paragraph/heading where it normalizes at parse-input level.
    ///
    /// **F5 Pass-1 correction (F-001):** the original test was misnamed
    /// `test_push_text_normalizes_crlf_in_paragraph` but actually tested a fenced
    /// codeBlock. Renamed to accurately describe the covered path.
    ///
    /// Red Gate: FAILS before the fix (push_text passes `\r` through in fenced codeBlock).
    /// Green Gate: passes after push_text gains context-aware CR-normalization.
    #[test]
    fn test_push_text_normalizes_lone_cr_in_fenced_code_block() {
        // Fenced codeBlock with a lone \r in the body. pulldown passes this through
        // verbatim into Event::Text; inside CodeBlock context \r must become \n.
        let adf = markdown_to_adf("```\na\rb\n```").unwrap();
        let texts = collect_all_text_nodes(&adf);
        assert!(
            texts.iter().all(|t| !t.contains('\r')),
            "push_text must normalize lone \\r in fenced codeBlock content; \
             text nodes must contain no raw \\r (BC-7.2.011 EC-11)"
        );
        // Inside codeBlock the normalized form must be \n (not space), so multi-line
        // code retains its line structure.
        assert!(
            texts.iter().any(|t| t.contains("a\nb")),
            "push_text must normalize lone \\r to \\n inside codeBlock; \
             expected a text node containing \"a\\nb\" but got: {:?} (BC-7.2.011 EC-11)",
            texts
        );
    }

    /// BC-7.2.011 EC-11 — CRLF two-pass ordering pinned by direct `push_text`/`push_code`
    /// calls, bypassing pulldown.
    ///
    /// The normalization is `\r\n`→replacement THEN lone `\r`→replacement (two-pass).
    /// Incorrect ordering (lone `\r` first, then `\r\n`) would double-convert CRLF:
    ///   `\r\n` → `\n\n` (lone `\r` becomes `\n`, then the original `\n` stays) — wrong.
    ///
    /// Context rules (empirically confirmed, F5 Pass-1):
    /// - Inside codeBlock:    `\r\n`→`\n`,     lone `\r`→`\n`
    /// - Outside codeBlock:   `\r\n`→ space,   lone `\r`→ space   (INV-1: no raw `\n`)
    ///
    /// **F5 Pass-1 (F-001):** this is a new deterministic test that pins the two-pass
    /// ordering and the non-codeBlock space-substitution path that was missing before.
    #[test]
    fn test_push_text_crlf_two_pass_ordering_deterministic() {
        // --- Non-codeBlock path (paragraph) ---
        // "hello\r\nworld": CRLF → space (not \n, not doubled)
        {
            let mut b = AdfBuilder::new();
            b.push(NodeKind::Paragraph);
            b.push_text("hello\r\nworld");
            b.end(TagEnd::Paragraph);
            let doc = json!({"version":1,"type":"doc","content":b.root});
            let texts = collect_all_text_nodes(&doc);
            assert!(
                texts.iter().any(|t| t == "hello world"),
                "CRLF in non-codeBlock must become space (two-pass: \\r\\n→space first); \
                 got: {:?}",
                texts
            );
        }
        // "a\r\rb": two lone CRs → two spaces (not doubled \n)
        {
            let mut b = AdfBuilder::new();
            b.push(NodeKind::Paragraph);
            b.push_text("a\r\rb");
            b.end(TagEnd::Paragraph);
            let doc = json!({"version":1,"type":"doc","content":b.root});
            let texts = collect_all_text_nodes(&doc);
            assert!(
                texts.iter().any(|t| t == "a  b"),
                "two lone CRs in non-codeBlock must become two spaces; got: {:?}",
                texts
            );
        }
        // "\r\n\r": CRLF + lone CR → two spaces
        {
            let mut b = AdfBuilder::new();
            b.push(NodeKind::Paragraph);
            b.push_text("\r\n\r");
            b.end(TagEnd::Paragraph);
            let doc = json!({"version":1,"type":"doc","content":b.root});
            let texts = collect_all_text_nodes(&doc);
            assert!(
                texts.iter().any(|t| t == "  "),
                "CRLF + lone CR in non-codeBlock must become two spaces; got: {:?}",
                texts
            );
        }
        // --- codeBlock path ---
        // "hello\r\nworld": CRLF → \n (not doubled)
        {
            let mut b = AdfBuilder::new();
            b.push(NodeKind::CodeBlock { language: None });
            b.push_text("hello\r\nworld");
            b.end(TagEnd::CodeBlock);
            let doc = json!({"version":1,"type":"doc","content":b.root});
            let texts = collect_all_text_nodes(&doc);
            assert!(
                texts.iter().any(|t| t == "hello\nworld"),
                "CRLF in codeBlock must become \\n (not doubled); got: {:?}",
                texts
            );
        }
        // "a\r\rb": two lone CRs → two \n chars
        {
            let mut b = AdfBuilder::new();
            b.push(NodeKind::CodeBlock { language: None });
            b.push_text("a\r\rb");
            b.end(TagEnd::CodeBlock);
            let doc = json!({"version":1,"type":"doc","content":b.root});
            let texts = collect_all_text_nodes(&doc);
            assert!(
                texts.iter().any(|t| t == "a\n\nb"),
                "two lone CRs in codeBlock must become two \\n chars; got: {:?}",
                texts
            );
        }
        // "\r\n\r": CRLF + lone CR → two \n chars
        {
            let mut b = AdfBuilder::new();
            b.push(NodeKind::CodeBlock { language: None });
            b.push_text("\r\n\r");
            b.end(TagEnd::CodeBlock);
            let doc = json!({"version":1,"type":"doc","content":b.root});
            let texts = collect_all_text_nodes(&doc);
            assert!(
                texts.iter().any(|t| t == "\n\n"),
                "CRLF + lone CR in codeBlock must become two \\n chars; got: {:?}",
                texts
            );
        }
    }

    /// BC-7.2.011 EC-11 — `push_code` normalizes lone `\r` before building the
    /// inline code text node; CR becomes space (inline text, not codeBlock context).
    ///
    /// pulldown-cmark normalizes `\r` to a space inside inline code spans (CommonMark
    /// §6.3), so `markdown_to_adf("`a\rb`")` never routes a raw `\r` through
    /// `Event::Code`. The direct `push_code` path guards against any other caller
    /// supplying a raw `\r`.
    ///
    /// **F5 Pass-1 correction (F-002):** the original assertion
    /// `texts.iter().any(|t| t == "a\nb")` was wrong — inline code is NOT in a
    /// codeBlock context, so `\r` must become a space, producing `"a b"`, not `"a\nb"`.
    /// The `markdown_to_adf("`a\rb`")` assertion documented pulldown §6.3 behavior
    /// (which already produces `"a b"`) and was vacuous as a regression guard — it
    /// passes even without the fix. That assertion is retained with an explanatory
    /// comment distinguishing the pulldown path from the direct-call guard.
    ///
    /// Red Gate: FAILS before the fix (push_code passes `\r` through verbatim).
    /// Green Gate: passes after push_code gains the CR-normalization guard.
    #[test]
    fn test_push_code_normalizes_lone_cr_in_inline_code() {
        // Direct path: call push_code with a raw \r, bypassing pulldown-cmark.
        // Wrap in a paragraph so append_child has a parent to attach to.
        let mut builder = AdfBuilder::new();
        builder.push(NodeKind::Paragraph);
        builder.push_code("a\rb");
        builder.end(TagEnd::Paragraph);
        let doc = json!({
            "version": 1,
            "type": "doc",
            "content": builder.root,
        });
        let texts = collect_all_text_nodes(&doc);
        // push_code is inline (not inside a codeBlock), so lone \r must become space.
        assert!(
            texts.iter().any(|t| t == "a b"),
            "push_code must normalize lone \\r to space in inline context; \
             expected text node \"a b\" but got: {:?} (BC-7.2.011 EC-11)",
            texts
        );
        // No text node anywhere in the ADF may contain a raw \r.
        assert!(
            texts.iter().all(|t| !t.contains('\r')),
            "push_code must leave no raw \\r in any text node; \
             got: {:?} (BC-7.2.011 EC-11)",
            texts
        );
        // Documents pulldown CommonMark §6.3 behavior: pulldown converts inline-code
        // \r to a space BEFORE emitting Event::Code, so no \r reaches push_code on
        // this path. This assertion is NOT a regression guard for the push_code fix
        // (it passes even without the fix); it documents the pulldown §6.3 invariant.
        let via_md = markdown_to_adf("`a\rb`").unwrap();
        assert!(
            collect_all_text_nodes(&via_md)
                .iter()
                .all(|t| !t.contains('\r')),
            "pulldown §6.3 already normalizes \\r to space in inline code; \
             no raw \\r must survive (BC-7.2.011 EC-11)"
        );
    }

    // -----------------------------------------------------------------------
    // BC-7.2.011 EC-12 — `text_to_adf` plain-text CR/newline normalization
    // (#522 F4-ext Red Gate)
    //
    // These tests assert the postconditions from BC-7.2.011 EC-12 /
    // INV-1-plain-text.  `text_to_adf` currently emits a single text node
    // with the raw input string, so all multi-line inputs violate INV-1.
    //
    // Red Gate requirement (from story S-522 §Phase 1b):
    //   - test_text_to_adf_single_line_unchanged  → MAY PASS pre-fix (regression guard)
    //   - all other EC-12 tests                   → MUST FAIL pre-fix
    // -----------------------------------------------------------------------

    /// BC-7.2.011 EC-12 (AC-008) — single-line input is byte-identical to the
    /// pre-fix output.  No `\r`/`\n` in input → output shape is unchanged.
    ///
    /// Red Gate: PASSES pre-fix (single-line path is currently correct).
    /// This test is a no-regression pin: any future change that breaks
    /// single-line inputs will fail CI immediately.
    #[test]
    fn test_text_to_adf_single_line_unchanged() {
        let adf = text_to_adf("hello");
        // Exact ADF shape must be the current one-paragraph / one-text-node form.
        assert_eq!(
            adf["type"], "doc",
            "text_to_adf single-line: root must be doc (BC-7.2.011 EC-12 AC-008)"
        );
        assert_eq!(
            adf["version"], 1,
            "text_to_adf single-line: version must be 1 (BC-7.2.011 EC-12 AC-008)"
        );
        let content = adf["content"]
            .as_array()
            .expect("doc.content must be an array");
        assert_eq!(
            content.len(),
            1,
            "text_to_adf single-line: doc must have exactly one paragraph; \
             got: {:?} (BC-7.2.011 EC-12 AC-008)",
            content
        );
        assert_eq!(
            content[0]["type"], "paragraph",
            "text_to_adf single-line: first child must be paragraph (BC-7.2.011 EC-12 AC-008)"
        );
        let para_content = content[0]["content"]
            .as_array()
            .expect("paragraph.content must be an array");
        assert_eq!(
            para_content.len(),
            1,
            "text_to_adf single-line: paragraph must have exactly one text node; \
             got: {:?} (BC-7.2.011 EC-12 AC-008)",
            para_content
        );
        assert_eq!(
            para_content[0]["type"], "text",
            "text_to_adf single-line: content[0] must be a text node (BC-7.2.011 EC-12 AC-008)"
        );
        assert_eq!(
            para_content[0]["text"], "hello",
            "text_to_adf single-line: text node must contain the exact input string \
             (BC-7.2.011 EC-12 AC-008)"
        );
        // INV-1: no raw \r or \n in any text node (trivially holds for single-line input).
        assert_no_raw_newline_in_text_nodes(&adf, "hello");
    }

    /// BC-7.2.011 EC-12 (AC-009) — interior LF becomes a `hardBreak` node;
    /// no raw `\n` survives into any text node.
    ///
    /// Expected ADF after fix:
    ///   doc > [paragraph > [text("line1"), hardBreak, text("line2")]]
    ///
    /// Red Gate: FAILS pre-fix — current `text_to_adf` places the raw `\n`
    /// verbatim in a single text node, violating INV-1.
    #[test]
    fn test_text_to_adf_normalizes_interior_lf_to_hardbreak() {
        let input = "line1\nline2";
        let adf = text_to_adf(input);

        // The doc must have exactly one paragraph.
        let content = adf["content"]
            .as_array()
            .expect("doc.content must be an array");
        assert_eq!(
            content.len(),
            1,
            "text_to_adf(\"line1\\nline2\"): expected 1 paragraph, got {} \
             (BC-7.2.011 EC-12 AC-009)",
            content.len()
        );
        assert_eq!(
            content[0]["type"], "paragraph",
            "text_to_adf interior LF: first child must be paragraph (BC-7.2.011 EC-12 AC-009)"
        );

        let para = content[0]["content"]
            .as_array()
            .expect("paragraph.content must be an array");

        // Expected: [text("line1"), hardBreak, text("line2")]
        assert_eq!(
            para.len(),
            3,
            "text_to_adf(\"line1\\nline2\"): paragraph must have 3 children \
             [text, hardBreak, text]; got {:?} (BC-7.2.011 EC-12 AC-009)",
            para
        );
        assert_eq!(
            para[0]["type"], "text",
            "text_to_adf interior LF: para[0] must be text (BC-7.2.011 EC-12 AC-009)"
        );
        assert_eq!(
            para[0]["text"], "line1",
            "text_to_adf interior LF: first text node must be \"line1\" \
             (BC-7.2.011 EC-12 AC-009)"
        );
        assert_eq!(
            para[1]["type"], "hardBreak",
            "text_to_adf interior LF: para[1] must be hardBreak \
             (BC-7.2.011 EC-12 AC-009)"
        );
        assert_eq!(
            para[2]["type"], "text",
            "text_to_adf interior LF: para[2] must be text (BC-7.2.011 EC-12 AC-009)"
        );
        assert_eq!(
            para[2]["text"], "line2",
            "text_to_adf interior LF: second text node must be \"line2\" \
             (BC-7.2.011 EC-12 AC-009)"
        );

        // INV-1: no raw \n in any text node.
        assert_no_raw_newline_in_text_nodes(&adf, input);
    }

    /// BC-7.2.011 EC-12 (AC-010) — interior CRLF (`\r\n`) is normalized to a
    /// single `hardBreak`, the same shape as the interior-LF case (AC-009).
    /// Two-pass ordering: `\r\n`→`\n` first, then split on `\n` → one boundary.
    ///
    /// Red Gate: FAILS pre-fix — raw `\r\n` embedded in single text node.
    #[test]
    fn test_text_to_adf_normalizes_interior_crlf_to_hardbreak() {
        let input = "line1\r\nline2";
        let adf = text_to_adf(input);

        let content = adf["content"]
            .as_array()
            .expect("doc.content must be an array");
        assert_eq!(
            content.len(),
            1,
            "text_to_adf(\"line1\\r\\nline2\"): expected 1 paragraph, got {} \
             (BC-7.2.011 EC-12 AC-010)",
            content.len()
        );

        let para = content[0]["content"]
            .as_array()
            .expect("paragraph.content must be an array");

        // Same shape as AC-009: [text("line1"), hardBreak, text("line2")]
        assert_eq!(
            para.len(),
            3,
            "text_to_adf(\"line1\\r\\nline2\"): paragraph must have 3 children; \
             got {:?} (BC-7.2.011 EC-12 AC-010)",
            para
        );
        assert_eq!(
            para[0]["text"], "line1",
            "text_to_adf CRLF: first text node (BC-7.2.011 EC-12 AC-010)"
        );
        assert_eq!(
            para[1]["type"], "hardBreak",
            "text_to_adf CRLF: middle must be hardBreak (BC-7.2.011 EC-12 AC-010)"
        );
        assert_eq!(
            para[2]["text"], "line2",
            "text_to_adf CRLF: second text node (BC-7.2.011 EC-12 AC-010)"
        );

        // INV-1: no raw \r or \n in any text node.
        assert_no_raw_newline_in_text_nodes(&adf, input);
    }

    /// BC-7.2.011 EC-12 (AC-009) — three lines separated by single `\n` produce
    /// exactly two interior `hardBreak` nodes, one between each adjacent pair of
    /// text segments.  The existing two-line tests (AC-009/010/011) only exercise
    /// `len == 2`, where the hardBreak-insertion guard `i < len - 1` is
    /// indistinguishable from off-by-one variants that the trailing-hardBreak
    /// trim then masks.  A three-segment block is the smallest input that pins the
    /// per-pair break count: `[text(a), hardBreak, text(b), hardBreak, text(c)]`.
    ///
    /// F6 (S-522): kills the `i < len - 1` → `i < len / 2` mutant in `text_to_adf`,
    /// which for `len == 3` would emit only one break and fuse `b`/`c` into
    /// adjacent text nodes with no separator.
    #[test]
    fn test_text_to_adf_three_lines_produce_two_interior_hardbreaks() {
        let input = "a\nb\nc";
        let adf = text_to_adf(input);

        let content = adf["content"]
            .as_array()
            .expect("doc.content must be an array");
        assert_eq!(
            content.len(),
            1,
            "text_to_adf(\"a\\nb\\nc\"): expected 1 paragraph, got {} \
             (BC-7.2.011 EC-12 AC-009)",
            content.len()
        );

        let para = content[0]["content"]
            .as_array()
            .expect("paragraph.content must be an array");

        // Expected: [text(a), hardBreak, text(b), hardBreak, text(c)] — exactly
        // two interior hardBreaks (one per adjacent pair of non-empty segments).
        assert_eq!(
            para.len(),
            5,
            "text_to_adf(\"a\\nb\\nc\"): paragraph must have 5 children \
             [text, hardBreak, text, hardBreak, text]; got {para:?} \
             (BC-7.2.011 EC-12 AC-009)"
        );
        assert_eq!(para[0]["text"], "a", "para[0] text (AC-009)");
        assert_eq!(para[1]["type"], "hardBreak", "para[1] hardBreak (AC-009)");
        assert_eq!(para[2]["text"], "b", "para[2] text (AC-009)");
        assert_eq!(para[3]["type"], "hardBreak", "para[3] hardBreak (AC-009)");
        assert_eq!(para[4]["text"], "c", "para[4] text (AC-009)");

        let hardbreak_count = para.iter().filter(|n| n["type"] == "hardBreak").count();
        assert_eq!(
            hardbreak_count, 2,
            "text_to_adf(\"a\\nb\\nc\"): expected exactly 2 interior hardBreaks, \
             got {hardbreak_count} (BC-7.2.011 EC-12 AC-009)"
        );

        // INV-1: no raw \n in any text node.
        assert_no_raw_newline_in_text_nodes(&adf, input);
    }

    /// BC-7.2.011 EC-12 (AC-011) — interior lone `\r` (old-Mac line ending) is
    /// normalized to a `hardBreak`, the same shape as the interior-LF case (AC-009).
    ///
    /// Red Gate: FAILS pre-fix — raw `\r` embedded in single text node.
    #[test]
    fn test_text_to_adf_normalizes_interior_lone_cr_to_hardbreak() {
        let input = "line1\rline2";
        let adf = text_to_adf(input);

        let content = adf["content"]
            .as_array()
            .expect("doc.content must be an array");
        assert_eq!(
            content.len(),
            1,
            "text_to_adf(\"line1\\rline2\"): expected 1 paragraph, got {} \
             (BC-7.2.011 EC-12 AC-011)",
            content.len()
        );

        let para = content[0]["content"]
            .as_array()
            .expect("paragraph.content must be an array");

        // Same shape as AC-009: [text("line1"), hardBreak, text("line2")]
        assert_eq!(
            para.len(),
            3,
            "text_to_adf(\"line1\\rline2\"): paragraph must have 3 children; \
             got {:?} (BC-7.2.011 EC-12 AC-011)",
            para
        );
        assert_eq!(
            para[0]["text"], "line1",
            "text_to_adf lone CR: first text node (BC-7.2.011 EC-12 AC-011)"
        );
        assert_eq!(
            para[1]["type"], "hardBreak",
            "text_to_adf lone CR: middle must be hardBreak (BC-7.2.011 EC-12 AC-011)"
        );
        assert_eq!(
            para[2]["text"], "line2",
            "text_to_adf lone CR: second text node (BC-7.2.011 EC-12 AC-011)"
        );

        // INV-1: no raw \r or \n in any text node.
        assert_no_raw_newline_in_text_nodes(&adf, input);
    }

    /// BC-7.2.011 EC-12 (AC-012) — trailing newlines are stripped before any
    /// normalization; output is identical to `text_to_adf("hello")`.
    ///
    /// Red Gate: FAILS pre-fix for all three trailing-newline variants — raw
    /// `\n`, `\r\n`, or `\r` embedded in the text node.
    #[test]
    fn test_text_to_adf_strips_trailing_newlines() {
        let expected = text_to_adf("hello");

        for (input, label) in [
            ("hello\n", "LF"),
            ("hello\r\n", "CRLF"),
            ("hello\r", "lone CR"),
        ] {
            let adf = text_to_adf(input);
            assert_eq!(
                adf, expected,
                "text_to_adf trailing {} must produce same output as text_to_adf(\"hello\"); \
                 got: {} (BC-7.2.011 EC-12 AC-012)",
                label, adf
            );
            assert_no_raw_newline_in_text_nodes(&adf, input);
        }
    }

    /// BC-7.2.011 EC-12 (AC-013) — a blank line (`\n\n`) splits the input into
    /// two separate `paragraph` nodes; consecutive blank lines (`\n\n\n`) collapse
    /// to one paragraph boundary (same two-paragraph output).
    ///
    /// Red Gate: FAILS pre-fix — raw `\n\n` embedded in single text node.
    #[test]
    fn test_text_to_adf_blank_line_produces_two_paragraphs() {
        for (input, label) in [
            ("line1\n\nline2", "double LF"),
            ("line1\n\n\nline2", "triple LF"),
        ] {
            let adf = text_to_adf(input);
            let content = adf["content"]
                .as_array()
                .expect("doc.content must be an array");

            assert_eq!(
                content.len(),
                2,
                "text_to_adf({label:?}): expected 2 paragraph nodes (blank line splits), \
                 got {} — content: {:?} (BC-7.2.011 EC-12 AC-013)",
                content.len(),
                content
            );
            assert_eq!(
                content[0]["type"], "paragraph",
                "text_to_adf({label:?}): content[0] must be paragraph \
                 (BC-7.2.011 EC-12 AC-013)"
            );
            assert_eq!(
                content[1]["type"], "paragraph",
                "text_to_adf({label:?}): content[1] must be paragraph \
                 (BC-7.2.011 EC-12 AC-013)"
            );

            // Text values in each paragraph.
            let p0 = &content[0]["content"][0];
            assert_eq!(
                p0["text"], "line1",
                "text_to_adf({label:?}): first paragraph text (BC-7.2.011 EC-12 AC-013)"
            );
            let p1 = &content[1]["content"][0];
            assert_eq!(
                p1["text"], "line2",
                "text_to_adf({label:?}): second paragraph text (BC-7.2.011 EC-12 AC-013)"
            );

            // INV-1: no raw \n or \r in any text node.
            assert_no_raw_newline_in_text_nodes(&adf, input);
        }
    }

    /// BC-7.2.011 EC-12 (AC-014) — property-style: `assert_no_raw_newline_in_text_nodes`
    /// passes for a representative sample of multi-line inputs.  Every input in the
    /// sample has at least one `\r` or `\n`; all MUST FAIL pre-fix.
    ///
    /// Red Gate: FAILS pre-fix for ALL multi-line inputs (raw newlines embedded in
    /// the single text node produced by the pre-fix one-liner).
    #[test]
    fn test_text_to_adf_no_raw_newline_in_any_text_node() {
        // Representative sample from AC-014 (story S-522).
        let inputs: &[&str] = &[
            "a\nb",         // interior LF
            "a\r\nb",       // interior CRLF
            "a\rb",         // interior lone CR
            "a\n\nb",       // blank-line boundary
            "a\r\n\r\nb",   // CRLF blank line
            "a\nb\n\nc\nd", // mixed interior LF + blank-line boundary
            "\n\n\n",       // all newlines → empty/stripped input
        ];

        for &input in inputs {
            let adf = text_to_adf(input);
            // INV-1 via existing helper: no raw \n in non-codeBlock text nodes,
            // no raw \r in any text node.  (text_to_adf produces no codeBlock,
            // so the non-codeBlock \n check is unconditional for all its output.)
            assert_no_raw_newline_in_text_nodes(&adf, input);
        }
    }

    /// BC-7.2.011 EC-12 — pin the exact JSON shape that `text_to_adf("")`
    /// produces (F5 finding F-3).
    ///
    /// `text_to_adf("")` takes the fast path (no `\r`/`\n`) and returns a
    /// single paragraph containing a single empty-string text node:
    ///
    ///   doc > [paragraph > [text("")]]
    ///
    /// `assert_no_raw_newline_in_text_nodes` passes trivially on this output
    /// because the text node value is `""` (no control chars) — so the
    /// INV-1 helper provides no shape guarantee here.  This test pins the
    /// exact structure with `assert_eq!` so a regression that changes the
    /// shape (e.g. emitting no text node, or a `null` content) is caught.
    #[test]
    fn test_text_to_adf_empty_string_shape() {
        let adf = text_to_adf("");
        let expected = serde_json::json!({
            "version": 1,
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        { "type": "text", "text": "" }
                    ]
                }
            ]
        });
        assert_eq!(
            adf, expected,
            "text_to_adf(\"\") must produce doc > [paragraph > [text(\"\")]] \
             (BC-7.2.011 EC-12 F5-F3)"
        );
    }

    /// BC-7.2.011 EC-12 — pin the exact JSON shape that `text_to_adf("\n\n\n")`
    /// produces (F5 finding F-3).
    ///
    /// All-newlines input: Step 2 strips all trailing `\r`/`\n`, leaving `""`;
    /// the block split produces `[""]`; every segment is empty → filter returns
    /// `None`; `paragraphs.is_empty()` → fallback produces the same shape as
    /// `text_to_adf("")`:
    ///
    ///   doc > [paragraph > [text("")]]
    ///
    /// `assert_no_raw_newline_in_text_nodes` passes trivially because the text
    /// node value is `""` — this explicit shape test is the operative pin.
    #[test]
    fn test_text_to_adf_all_newlines_shape() {
        let adf = text_to_adf("\n\n\n");
        let expected = serde_json::json!({
            "version": 1,
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        { "type": "text", "text": "" }
                    ]
                }
            ]
        });
        assert_eq!(
            adf, expected,
            "text_to_adf(\"\\n\\n\\n\") must produce doc > [paragraph > [text(\"\")]] \
             (BC-7.2.011 EC-12 F5-F3)"
        );
    }

    /// BC-7.2.011 EC-12 — pin the exact JSON shape that
    /// `text_to_adf("a\n \nb")` produces (OBS-2 coverage).
    ///
    /// The middle line is a single space (NOT empty), so the input contains
    /// no `"\n\n"` blank-line separator — the whole input is ONE block.
    /// Within that block, `split('\n')` yields three segments: `"a"`, `" "`,
    /// `"b"`.  The space segment is non-empty, so it becomes a `text(" ")`
    /// node rather than being dropped.  Expected shape:
    ///
    ///   doc > [paragraph > [text("a"), hardBreak, text(" "), hardBreak, text("b")]]
    ///
    /// INV-1 holds: no text node contains a raw `\n` or `\r`.
    #[test]
    fn test_text_to_adf_whitespace_only_blank_line_preserved() {
        let adf = text_to_adf("a\n \nb");
        let expected = serde_json::json!({
            "version": 1,
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        { "type": "text", "text": "a" },
                        { "type": "hardBreak" },
                        { "type": "text", "text": " " },
                        { "type": "hardBreak" },
                        { "type": "text", "text": "b" }
                    ]
                }
            ]
        });
        assert_eq!(
            adf, expected,
            "text_to_adf(\"a\\n \\nb\") must produce a single paragraph with \
             [text(\"a\"), hardBreak, text(\" \"), hardBreak, text(\"b\")] — \
             whitespace-only middle segment must be preserved as text, \
             not treated as a blank-line paragraph separator \
             (BC-7.2.011 EC-12 OBS-2)"
        );
    }

    /// BC-7.2.011 EC-12 (AC-014 optional) — proptest: INV-1 holds for
    /// `text_to_adf` over arbitrary string inputs.
    ///
    /// Strategy: explicit charset `[\r\n\t a-zA-Z0-9]{0,64}` ensures that
    /// `\r` and `\n` (the INV-1 control chars) are always in the sample
    /// space.  The former `".*"` strategy silently excluded `\n` because
    /// Rust regex `.` does not match newlines by default (F5 finding F-1).
    ///
    /// Generates 1000+ random strings and asserts that no text node in the
    /// returned ADF contains a raw `\r` or non-codeBlock `\n`.
    ///
    /// Red Gate: FAILS pre-fix for any input containing `\r` or `\n`.
    #[test]
    fn prop_text_to_adf_holds_inv1() {
        let config = proptest::test_runner::Config {
            cases: 1000,
            ..proptest::test_runner::Config::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(config);
        runner
            .run(
                &proptest::string::string_regex("[\\r\\n\\t a-zA-Z0-9]{0,64}").unwrap(),
                |input| {
                    let adf = text_to_adf(&input);
                    assert_no_raw_newline_in_text_nodes(&adf, &input);
                    Ok(())
                },
            )
            .unwrap();
    }

    // -----------------------------------------------------------------------
    // BC-7.2.011 EC-11 (CR-01 red gate, #522 F5-INV-1 gap)
    //
    // `push_text` and `push_code` only normalize control chars when
    // `text.contains('\r')`.  A bare `\n` (no `\r`) in Other context
    // (paragraph, heading, inline-mark stack top — anything that is NOT
    // a codeBlock or HtmlBlock) passes through unchanged, embedding a raw
    // `\n` in the emitted text node.  This violates BC-7.2.011 INV-1 and
    // causes Jira HTTP 400.
    //
    // The fix will extend the Other-context branch to also map bare `\n`→space
    // (mirroring SoftBreak→" ").  CodeBlock MUST keep `\n` byte-identical.
    //
    // Red Gate requirement:
    //   test 1 (push_text Other bare \n)      → MUST FAIL pre-fix
    //   test 2 (push_code bare \n)            → MUST FAIL pre-fix
    //   test 3 (push_text CodeBlock \n)       → MUST PASS pre-fix AND post-fix
    //   test 4 (markdown inline-HTML \n)      → EMPIRICALLY determined below
    //   test 5 (proptest markdown HTML chars) → likely FAILS pre-fix if test 4 RED
    //   test 6 (text_to_adf worklog multiline)→ MUST PASS pre-fix AND post-fix
    // -----------------------------------------------------------------------

    /// BC-7.2.011 EC-11 (CR-01, #522 test 1) — `push_text` in Other context
    /// (paragraph) must normalize a bare `\n` to a space, not pass it through.
    ///
    /// Contract: bare `\n` in non-codeBlock, non-HtmlBlock context (i.e.
    /// the `Other` branch) must become a space, mirroring SoftBreak→" ".
    /// The emitted text node must contain `"a b"`, not `"a\nb"`.
    ///
    /// Red Gate: FAILS before the fix because `push_text` only runs the
    /// normalization block when `text.contains('\r')`, leaving a bare `\n`
    /// untouched → raw `\n` in the text node → INV-1 violation.
    #[test]
    fn test_push_text_normalizes_bare_lf_in_other_context_to_space() {
        // Construct AdfBuilder with a paragraph (Other context) directly.
        // Do NOT use markdown_to_adf — we need to call push_text with a raw \n
        // to bypass pulldown-cmark's own normalization.
        let mut builder = AdfBuilder::new();
        builder.push(NodeKind::Paragraph);
        builder.push_text("a\nb");
        builder.end(TagEnd::Paragraph);

        let doc = json!({
            "version": 1,
            "type": "doc",
            "content": builder.root,
        });
        let texts = collect_all_text_nodes(&doc);

        // Post-fix expectation: bare \n → single space → "a b".
        assert!(
            texts.iter().any(|t| t == "a b"),
            "push_text must normalize bare \\n to space in Other (paragraph) context \
             (BC-7.2.011 EC-11 CR-01 #522 test 1); expected text node \"a b\" but got: {:?}",
            texts
        );

        // Enforce INV-1 directly: no raw \n must survive.
        assert!(
            texts.iter().all(|t| !t.contains('\n')),
            "push_text must not leave raw \\n in any text node in Other context \
             (BC-7.2.011 EC-11 INV-1 #522 test 1); got: {:?}",
            texts
        );

        // Belt-and-suspenders: the full INV-1 helper (also checks \r).
        assert_no_raw_newline_in_text_nodes(&doc, "a\nb");
    }

    /// BC-7.2.011 EC-11 (CR-01, #522 test 2) — `push_code` must normalize
    /// a bare `\n` to a space before building the inline code text node.
    ///
    /// `push_code` always emits an inline code mark — the `code` mark text
    /// node is never inside a block-level codeBlock, so the Other-context
    /// rule applies: `\n`→space, same as `\r`→space.
    ///
    /// Red Gate: FAILS before the fix because `push_code` only runs the
    /// normalization when `text.contains('\r')`.
    #[test]
    fn test_push_code_normalizes_bare_lf_to_space() {
        // Wrap in a paragraph so append_child has a parent.
        let mut builder = AdfBuilder::new();
        builder.push(NodeKind::Paragraph);
        builder.push_code("a\nb");
        builder.end(TagEnd::Paragraph);

        let doc = json!({
            "version": 1,
            "type": "doc",
            "content": builder.root,
        });
        let texts = collect_all_text_nodes(&doc);

        // Post-fix expectation: bare \n in inline code → space → "a b".
        assert!(
            texts.iter().any(|t| t == "a b"),
            "push_code must normalize bare \\n to space in inline context \
             (BC-7.2.011 EC-11 CR-01 #522 test 2); expected text node \"a b\" but got: {:?}",
            texts
        );

        // INV-1: no raw \n in any text node.
        assert!(
            texts.iter().all(|t| !t.contains('\n')),
            "push_code must not leave raw \\n in any text node \
             (BC-7.2.011 EC-11 INV-1 #522 test 2); got: {:?}",
            texts
        );
    }

    /// BC-7.2.011 EC-11 (CR-01, #522 test 3) — regression guard: `push_text`
    /// inside a CodeBlock context must preserve bare `\n` byte-identical.
    ///
    /// codeBlock content is preformatted — multi-line code is legally stored
    /// as a single text node with literal `\n` newlines, which Jira accepts.
    /// The fix must NOT touch the CodeBlock branch.
    ///
    /// This test MUST PASS both before and after the fix.
    #[test]
    fn test_push_text_codeblock_preserves_bare_lf() {
        let mut builder = AdfBuilder::new();
        builder.push(NodeKind::CodeBlock { language: None });
        builder.push_text("a\nb");
        builder.end(TagEnd::CodeBlock);

        let doc = json!({
            "version": 1,
            "type": "doc",
            "content": builder.root,
        });
        let texts = collect_all_text_nodes(&doc);

        // codeBlock \n must be preserved as-is — NOT converted to a space.
        assert!(
            texts.iter().any(|t| t == "a\nb"),
            "push_text must preserve bare \\n inside a codeBlock (Jira accepts \
             multi-line preformatted text); expected text node \"a\\nb\" but got: {:?} \
             (BC-7.2.011 EC-11 #522 test 3 regression guard)",
            texts
        );

        // Confirm codeBlock text contains the literal \n (not stripped or replaced).
        assert!(
            texts.iter().any(|t| t.contains('\n')),
            "codeBlock text node must still contain the raw \\n character after the fix \
             (BC-7.2.011 EC-11 #522 test 3 regression guard); got: {:?}",
            texts
        );
    }

    /// BC-7.2.011 EC-11 (CR-01, #522 test 4) — reachability probe: does
    /// multi-line INLINE HTML inside a paragraph reach `push_text` with an
    /// embedded `\n`, violating INV-1?
    ///
    /// Background: inline HTML (`Event::InlineHtml`) is handled via the shared
    /// `push_text` arm in the pulldown event loop.  pulldown-cmark follows
    /// CommonMark §6.6: inline HTML spans are arbitrary tag-like fragments
    /// within a paragraph (not block HTML).  Unlike block HTML (`Tag::HtmlBlock`
    /// whose Algorithm B explicitly normalises newlines), inline HTML text passes
    /// through `push_text` directly.  If pulldown emits an `Event::InlineHtml`
    /// carrying an embedded `\n`, it reaches the `Other` branch of `push_text`
    /// and — absent the fix — lands in the text node verbatim.
    ///
    /// This test feeds `markdown_to_adf` inputs with HTML-tag-like fragments
    /// that contain a `\n` inside the tag attribute position and asserts
    /// `assert_no_raw_newline_in_text_nodes` on the result.
    ///
    /// EMPIRICAL DETERMINATION (pre-fix):
    /// If this test FAILS before the fix → CR-01 is reachable end-to-end via
    ///   the markdown_to_adf → pulldown → Event::InlineHtml → push_text path
    ///   (HIGH severity — exploitable through any CLI --description flag).
    /// If this test PASSES before the fix → CR-01 is a latent / defense-in-depth
    ///   gap (pulldown normalises newlines inside inline HTML before emitting
    ///   Event::InlineHtml) — the direct push_text/push_code tests (tests 1 & 2)
    ///   still justify the fix as a defense-in-depth invariant enforcement.
    ///
    /// The test is kept regardless of reachability verdict as a regression guard.
    #[test]
    fn test_markdown_multiline_inline_html_holds_inv1() {
        // Several shapes of inline HTML containing a \n, to probe whether
        // pulldown emits the \n or normalises it before Event::InlineHtml.
        let inputs: &[&str] = &[
            // \n inside a tag attribute
            "foo <span\ndata-x=\"1\">bar",
            // \n between tag name and >
            "a <b\n>c",
            // \n in a self-closing tag
            "x <br\n/>y",
            // multiple inline HTML fragments with \n
            "text <em\nclass=\"a\">more</em>",
            // 3+-line inline HTML: INV-1 must hold across all three interior \n (F-522-02)
            "a <b\nc\nd>e",
            // CRLF inline HTML: \r\n must not produce a raw \r or \n in any text node (F-522-02)
            "a <b\r\nc>d",
        ];

        for &input in inputs {
            let adf = markdown_to_adf(input).unwrap();
            // If this assertion fires on ANY input before the fix, CR-01 is
            // reachable end-to-end through markdown_to_adf (HIGH verdict).
            assert_no_raw_newline_in_text_nodes(
                &adf,
                input,
                // The assert_no_raw_newline_in_text_nodes signature takes (adf, input).
            );
        }
    }

    /// BC-7.2.011 EC-11 (CR-01, #522 test 5) — proptest: INV-1 holds for
    /// `markdown_to_adf` over inputs that include HTML-like chars (`<`, `>`,
    /// `/`, `"`, `=`) interleaved with `\r`/`\n`.
    ///
    /// The existing `prop_492_arbitrary_string_holds_core_invariants` proptest
    /// uses charset `[\r\n\t a-zA-Z0-9]{0,64}` which deliberately excludes `<`
    /// and `>`.  This test adds a new strategy that includes HTML-structural
    /// chars so the inline-HTML→push_text path is fuzzed.
    ///
    /// If CR-01 is reachable through pulldown (test 4 RED), this proptest will
    /// also be RED (and will find minimal failing inputs automatically).
    /// If CR-01 is latent (test 4 GREEN), this proptest may still be RED if
    /// there are other paths through the markdown parser that deliver raw `\n`
    /// to push_text Other context.
    ///
    /// Uses 1024 cases (fast CI while still dense sampling).
    #[test]
    fn prop_markdown_to_adf_html_chars_holds_inv1() {
        let config = proptest::test_runner::Config {
            cases: 1024,
            ..proptest::test_runner::Config::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(config);
        runner
            .run(
                // Extend the charset with HTML-structural chars so inline-HTML
                // fragments (e.g. "<br\n/>") can be generated.
                &proptest::string::string_regex("[\\r\\n\\t a-zA-Z0-9<>/\"=]{0,64}").unwrap(),
                |input| {
                    let adf = markdown_to_adf(&input).unwrap();
                    // INV-1: no raw \n in non-codeBlock text nodes, no raw \r anywhere.
                    assert_no_raw_newline_in_text_nodes(&adf, &input);
                    Ok(())
                },
            )
            .unwrap();
    }

    /// BC-7.2.011 EC-11 (CR-01, #522 test 6) — regression guard: `text_to_adf`
    /// (the worklog path, called at `src/cli/worklog.rs` via
    /// `message.map(adf::text_to_adf)`) correctly handles a multi-line message,
    /// producing hardBreak interior nodes with no raw `\n` in any text node.
    ///
    /// `text_to_adf` independently implements Algorithm B step logic and has its
    /// own multi-line path — it does NOT route through `push_text`.  This test
    /// confirms that the worklog path is already correct (INV-1 holds) and will
    /// continue to hold after the push_text fix (no regression).
    ///
    /// This test MUST PASS both before and after the push_text/push_code fix.
    #[test]
    fn test_text_to_adf_worklog_multiline_message_is_cr_lf_free() {
        // Simulate a worklog message with a bare \n (e.g. "Fixed the bug\nSee ticket").
        let adf = text_to_adf("a\nb");

        // INV-1: text_to_adf must never embed raw \n in a text node.
        assert_no_raw_newline_in_text_nodes(&adf, "a\nb");

        // Confirm the correct structural shape: one paragraph with
        // [text("a"), hardBreak, text("b")].
        let content = adf["content"]
            .as_array()
            .expect("text_to_adf must produce a doc with content array");
        assert_eq!(
            content.len(),
            1,
            "text_to_adf(\"a\\nb\") must produce one paragraph \
             (BC-7.2.011 EC-12, worklog path regression guard #522 test 6)"
        );
        let para_content = content[0]["content"]
            .as_array()
            .expect("paragraph must have a content array");
        // Expected: [text("a"), hardBreak, text("b")]
        assert_eq!(
            para_content.len(),
            3,
            "text_to_adf(\"a\\nb\") paragraph must have 3 inline nodes \
             [text, hardBreak, text] (BC-7.2.011 EC-12 #522 test 6); got: {:?}",
            para_content
        );
        assert_eq!(
            para_content[0]["type"], "text",
            "para_content[0] must be a text node (#522 test 6)"
        );
        assert_eq!(
            para_content[0]["text"], "a",
            "para_content[0].text must be \"a\" (#522 test 6)"
        );
        assert_eq!(
            para_content[1]["type"], "hardBreak",
            "para_content[1] must be hardBreak (interior \\n → hardBreak, not raw \\n) \
             (#522 test 6)"
        );
        assert_eq!(
            para_content[2]["type"], "text",
            "para_content[2] must be a text node (#522 test 6)"
        );
        assert_eq!(
            para_content[2]["text"], "b",
            "para_content[2].text must be \"b\" (#522 test 6)"
        );
    }

    // =========================================================================
    // SEC-001 — ADF recursion-depth guard (BC-7.2.012)
    //
    // RED GATE: These tests target the NEW fallible signatures:
    //   markdown_to_adf(&str) -> Result<Value, JrError>   (currently -> Value)
    //   adf_to_text(&Value)   -> Result<String, JrError>  (currently -> String)
    //
    // They will NOT COMPILE against the current infallible signatures.
    // That compile failure IS the red state — it will be resolved when the
    // implementer changes the production signatures.
    //
    // Depth boundary encoded:
    //   depth 255 → Ok  (one below the inclusive limit)
    //   depth 256 → Err (the boundary itself is rejected — inclusive 256)
    //   depth 257 → Err (guard fires at 256, never reaches 257)
    //
    // Safety note: Inputs use depth 255/256/257 only.  The current unguarded
    // implementation handles these depths without stack-overflowing (they are
    // far from the crash threshold).  No pathologically deep inputs are used.
    // =========================================================================

    // -------------------------------------------------------------------------
    // §9.4 Const-gate test (regression pin, SEC-001)
    // Pins MAX_ADF_DEPTH == 256 so a silent change must update this test.
    // This test compiles and runs today; it will FAIL until the implementer
    // adds `pub(crate) const MAX_ADF_DEPTH: usize = 256;` to src/adf.rs.
    // -------------------------------------------------------------------------

    #[test]
    fn test_max_adf_depth_constant_is_256() {
        // BC-7.2.012: The compile-time depth limit must be exactly 256.
        // If an implementer silently changes MAX_ADF_DEPTH this test catches it.
        assert_eq!(MAX_ADF_DEPTH, 256, "MAX_ADF_DEPTH must be 256 (SEC-001)");
    }

    // -------------------------------------------------------------------------
    // Helper: build a deeply-nested blockquote markdown string.
    //
    // Each level adds one `> ` prefix, producing markdown like:
    //   "> > > text"  (for depth 3)
    //
    // The leaf content is a plain paragraph.  This is the simplest nestable
    // construct that pulldown-cmark maps to a real ADF container node
    // (blockquote), ensuring the depth guard exercises a real structural nesting.
    // -------------------------------------------------------------------------

    fn make_nested_blockquote_markdown(depth: usize) -> String {
        // Build ">>> … text" with `depth` levels of blockquote nesting.
        // pulldown-cmark allows "> " (space optional) so we use "> " per level
        // which is unambiguous: depth=1 → "> text", depth=2 → "> > text", etc.
        let prefix = "> ".repeat(depth);
        format!("{}leaf content", prefix)
    }

    // -------------------------------------------------------------------------
    // Helper: build a deeply-nested ADF serde_json::Value.
    //
    // Produces a doc whose content is `depth` levels of nested blockquote nodes,
    // each containing one child, with a paragraph leaf at the bottom.
    //
    // This bypasses the forward path and tests the reverse (adf_to_text) guard
    // directly with a programmatically-constructed deep value.
    // -------------------------------------------------------------------------

    fn make_nested_adf_value(depth: usize) -> Value {
        // Build the leaf paragraph.
        let leaf = json!({
            "type": "paragraph",
            "content": [{"type": "text", "text": "deep"}]
        });
        // Wrap the leaf in `depth` blockquote containers.
        let mut node = leaf;
        for _ in 0..depth {
            node = json!({
                "type": "blockquote",
                "content": [node]
            });
        }
        json!({
            "version": 1,
            "type": "doc",
            "content": [node]
        })
    }

    // =========================================================================
    // §9.1 Forward direction — markdown_to_adf depth boundary tests
    //
    // NOTE: These tests call `markdown_to_adf(…)?` (Result<Value, JrError>)
    // which is the NEW fallible signature.  They WILL NOT COMPILE against
    // the current `-> Value` signature.  That IS the red state.
    // =========================================================================

    /// BC-7.2.012 forward path: depth 255 (one below the inclusive limit) is Ok.
    /// Node type: nested blockquote (SEC-001 §9.1).
    ///
    /// The post-build `autolink_bare_urls` pass walks the tree recursively,
    /// starting at depth=0 for the top-level content array and incrementing per
    /// container level.  For a document with N blockquotes the deepest content
    /// array seen is the paragraph's children at depth=N.  To keep the guard at
    /// depth 255 (OK, 255 < 256) we need N=254 blockquote wrappers: the guard
    /// sees depth 254 for the innermost blockquote content, then depth 255 for
    /// the paragraph content — both pass the `>= MAX_ADF_DEPTH` check.
    #[test]
    fn test_markdown_to_adf_depth_255_blockquote_is_ok() {
        let md = make_nested_blockquote_markdown(254);
        let result = markdown_to_adf(&md);
        assert!(
            result.is_ok(),
            "depth 255 (254-blockquote doc) must be Ok (SEC-001 §3); got: {result:?}"
        );
    }

    /// BC-7.2.012 forward path: deepest recursive node lands at EXACTLY depth 256
    /// → must be Err.  This pins the INCLUSIVE boundary and kills the `>=`→`>`
    /// mutant (CR-005, SEC-001).
    ///
    /// Depth accounting for `autolink_bare_urls` (same formula for
    /// `assign_local_ids_walk`, which runs first):
    ///   The function is called on the top-level content array at depth=0.
    ///   Each container node causes a recursive call on its `content` at depth+1.
    ///   For N blockquotes: blockquote_N.content is visited at depth=N, then
    ///   paragraph.content is visited at depth=N+1 (the deepest call).
    ///   Guard: `depth >= 256`.
    ///   N=255 → paragraph.content visited at depth 256 → fires `>=256` → Err.
    ///   N=254 → paragraph.content visited at depth 255 → passes → Ok (see above).
    ///
    /// Mutant kill proof: under `> 256` (wrong operator), N=255 gives deepest 256,
    /// which satisfies `256 > 256 == false` → the guard would NOT fire → Ok.
    /// This test asserts Err, so it FAILS under the mutant → mutant is killed.
    #[test]
    fn test_markdown_to_adf_deepest_node_at_256_is_err_boundary_exact() {
        // 255 blockquotes: deepest recursive call lands at depth 256.
        // Under `>= 256` (correct): Err.  Under `> 256` (mutant): Ok.  Test asserts Err.
        let md = make_nested_blockquote_markdown(255);
        let result = markdown_to_adf(&md);
        assert!(
            result.is_err(),
            "255-blockquote doc (deepest at depth 256) must be Err — pins inclusive boundary (CR-005)"
        );
        let err = result.unwrap_err();
        assert_eq!(
            err.exit_code(),
            64,
            "boundary-exact forward guard must produce exit_code 64; got {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("nesting too deep"),
            "error must contain 'nesting too deep'; got: {msg:?}"
        );
    }

    /// BC-7.2.012 forward path: depth 256 (the inclusive boundary) is Err.
    /// Node type: nested blockquote (SEC-001 §9.1).
    #[test]
    fn test_markdown_to_adf_depth_256_blockquote_is_err() {
        let md = make_nested_blockquote_markdown(256);
        let result = markdown_to_adf(&md);
        assert!(
            result.is_err(),
            "depth 256 blockquote must be Err (SEC-001 §3 inclusive boundary); got Ok"
        );
        let err = result.unwrap_err();
        assert_eq!(
            err.exit_code(),
            64,
            "depth guard must produce JrError with exit_code 64 (UserError); got {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("nesting too deep"),
            "error message must contain 'nesting too deep'; got: {msg:?}"
        );
        assert!(
            msg.contains("256"),
            "error message must contain '256' (the limit); got: {msg:?}"
        );
    }

    /// BC-7.2.012 forward path: depth 257 is Err (guard fires at 256, same error).
    /// Node type: nested blockquote (SEC-001 §9.1).
    #[test]
    fn test_markdown_to_adf_depth_257_blockquote_is_err() {
        let md = make_nested_blockquote_markdown(257);
        let result = markdown_to_adf(&md);
        assert!(
            result.is_err(),
            "depth 257 blockquote must be Err (guard fires at 256); got Ok"
        );
        let err = result.unwrap_err();
        assert_eq!(err.exit_code(), 64, "exit_code must be 64; got {err:?}");
        let msg = err.to_string();
        assert!(
            msg.contains("nesting too deep"),
            "error message must contain 'nesting too deep'; got: {msg:?}"
        );
    }

    // -------------------------------------------------------------------------
    // Helper: build deeply-nested list markdown.
    //
    // Nested lists in GFM require 2-space indentation per level.
    // depth=1 → "- leaf", depth=2 → "- x\n  - leaf", etc.
    //
    // This exercises a second node type (bulletList/listItem) to confirm the
    // depth guard is not node-type-specific.
    // -------------------------------------------------------------------------

    fn make_nested_list_markdown(depth: usize) -> String {
        if depth == 0 {
            return String::new();
        }
        // Build from outermost to innermost with correct GFM 2-space-per-level indentation.
        // For depth=3:
        //   - item
        //     - item
        //       - leaf
        let mut lines = Vec::with_capacity(depth);
        for level in 0..depth {
            let indent = "  ".repeat(level);
            if level < depth - 1 {
                lines.push(format!("{}- item", indent));
            } else {
                lines.push(format!("{}- leaf", indent));
            }
        }
        lines.join("\n")
    }

    /// BC-7.2.012 forward path: depth 256 nested lists → Err.
    /// Confirms the depth guard is not node-type-specific (SEC-001 §9.1).
    #[test]
    fn test_markdown_to_adf_depth_256_nested_list_is_err() {
        let md = make_nested_list_markdown(256);
        let result = markdown_to_adf(&md);
        assert!(
            result.is_err(),
            "depth 256 nested list must be Err (SEC-001 §3); got Ok"
        );
        let err = result.unwrap_err();
        assert_eq!(err.exit_code(), 64);
        let msg = err.to_string();
        assert!(
            msg.contains("nesting too deep"),
            "error message must contain 'nesting too deep'; got: {msg:?}"
        );
        assert!(
            msg.contains("256"),
            "error message must mention '256'; got: {msg:?}"
        );
    }

    /// BC-7.2.012 forward path: depth 257 nested lists → Err (different node type).
    #[test]
    fn test_markdown_to_adf_depth_257_nested_list_is_err() {
        let md = make_nested_list_markdown(257);
        let result = markdown_to_adf(&md);
        assert!(result.is_err(), "depth 257 nested list must be Err; got Ok");
        let err = result.unwrap_err();
        assert_eq!(err.exit_code(), 64);
    }

    /// BC-7.2.012 forward path: normal depth (real-world markdown) returns Ok
    /// and produces the expected structural output.  Regression guard: the guard
    /// must not alter happy-path behavior.
    #[test]
    fn test_markdown_to_adf_normal_depth_is_ok() {
        let md = "# Title\n\n- bullet one\n- bullet two\n\nSome **bold** text.\n";
        let result = markdown_to_adf(md);
        assert!(
            result.is_ok(),
            "normal-depth markdown must be Ok; got: {result:?}"
        );
        let adf = result.unwrap();
        assert_eq!(adf["type"], "doc");
        assert_eq!(adf["content"][0]["type"], "heading");
    }

    // =========================================================================
    // §9.2 Reverse direction — adf_to_text depth boundary tests
    //
    // NOTE: These tests call `adf_to_text(…)` returning Result<String, JrError>
    // (NEW fallible signature).  They WILL NOT COMPILE against the current
    // `-> String` signature.  That IS the red state.
    // =========================================================================

    /// BC-7.2.012 reverse path: ADF where the deepest render_node call is at
    /// depth 255 (one below the inclusive limit) is Ok.
    ///
    /// `render_node` is called with depth=0 for each top-level doc node.
    /// `render_children` increments depth by 1 before each child call.  For a
    /// document with N blockquotes the deepest call chain is:
    ///   render_node(bq_N, N-1) → render_children → render_node(paragraph, N)
    ///                          → render_children → render_node(text, N+1)
    /// The deepest `render_node` call depth is N+1 (for the leaf text node).
    /// To exercise exactly depth=255 we need N=254 blockquote wrappers:
    ///   text is called at depth 255 — 255 < 256, passes the guard. ✓
    #[test]
    fn test_adf_to_text_depth_255_is_ok() {
        // 254 blockquotes → text rendered at depth 255, which is below
        // MAX_ADF_DEPTH=256 (guard: depth >= 256 → Err).
        let adf = make_nested_adf_value(254);
        let result = adf_to_text(&adf);
        assert!(
            result.is_ok(),
            "depth 255 (254-blockquote ADF) must be Ok (SEC-001 §3); got: {result:?}"
        );
    }

    /// BC-7.2.012 reverse path: deepest `render_node` call lands at EXACTLY depth
    /// 256 → must be Err.  Pins the inclusive boundary, kills the `>=`→`>` mutant
    /// (CR-005, SEC-001).
    ///
    /// Depth accounting for `render_node`:
    ///   `adf_to_text` calls `render_node(child, 0)` for each top-level doc node.
    ///   `render_children(depth)` calls `render_node(child, depth + 1)` for each child.
    ///   For N blockquotes: chain is render_node(bq_1, 0) → render_node(bq_2, 1) →
    ///   … → render_node(bq_N, N-1) → render_node(paragraph, N) →
    ///   render_node(text, N+1).  Deepest call is at depth N+1.
    ///   Guard: `depth >= 256`.
    ///   N=255 → text rendered at depth 256 → fires `>=256` → Err.
    ///   N=254 → text rendered at depth 255 → passes → Ok (test above).
    ///
    /// Mutant kill proof: under `> 256` (wrong operator), N=255 gives deepest 256,
    /// which satisfies `256 > 256 == false` → guard does NOT fire → Ok.
    /// This test asserts Err → FAILS under the mutant → mutant is killed.
    #[test]
    fn test_adf_to_text_deepest_node_at_256_is_err_boundary_exact() {
        // 255 blockquotes: text node rendered at depth 256.
        // Under `>= 256` (correct): Err.  Under `> 256` (mutant): Ok.  Test asserts Err.
        let adf = make_nested_adf_value(255);
        let result = adf_to_text(&adf);
        assert!(
            result.is_err(),
            "255-blockquote ADF (text at depth 256) must be Err — pins inclusive boundary (CR-005)"
        );
        let err = result.unwrap_err();
        assert_eq!(
            err.exit_code(),
            64,
            "boundary-exact reverse guard must produce exit_code 64; got {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("nesting too deep"),
            "error must contain 'nesting too deep'; got: {msg:?}"
        );
    }

    /// BC-7.2.012 reverse path: ADF with 256 levels of nesting is Err
    /// (inclusive boundary, Option A — error).
    #[test]
    fn test_adf_to_text_depth_256_is_err() {
        let adf = make_nested_adf_value(256);
        let result = adf_to_text(&adf);
        assert!(
            result.is_err(),
            "depth 256 ADF must be Err (SEC-001 §3 inclusive boundary); got Ok"
        );
        let err = result.unwrap_err();
        assert_eq!(
            err.exit_code(),
            64,
            "reverse depth guard must produce JrError with exit_code 64; got {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("nesting too deep"),
            "reverse error message must contain 'nesting too deep'; got: {msg:?}"
        );
        assert!(
            msg.contains("256"),
            "reverse error message must contain '256'; got: {msg:?}"
        );
    }

    /// BC-7.2.012 reverse path: ADF with 257 levels → Err (guard fires at 256).
    #[test]
    fn test_adf_to_text_depth_257_is_err() {
        let adf = make_nested_adf_value(257);
        let result = adf_to_text(&adf);
        assert!(
            result.is_err(),
            "depth 257 ADF must be Err (guard fires at 256); got Ok"
        );
        let err = result.unwrap_err();
        assert_eq!(err.exit_code(), 64);
    }

    /// BC-7.2.012 reverse path: normal real-world ADF returns Ok.
    /// Regression guard: the guard must not alter happy-path rendering.
    #[test]
    fn test_adf_to_text_normal_depth_is_ok() {
        // Use a well-formed doc (one heading + one paragraph).
        // NEW SIGNATURE: will NOT COMPILE until signature change.
        let adf = json!({
            "version": 1,
            "type": "doc",
            "content": [
                {
                    "type": "heading",
                    "attrs": {"level": 2},
                    "content": [{"type": "text", "text": "Hello"}]
                },
                {
                    "type": "paragraph",
                    "content": [{"type": "text", "text": "world"}]
                }
            ]
        });
        let result = adf_to_text(&adf);
        assert!(result.is_ok(), "normal ADF must be Ok; got: {result:?}");
        let text = result.unwrap();
        assert!(
            text.contains("Hello"),
            "rendered text must contain 'Hello'; got: {text:?}"
        );
        assert!(
            text.contains("world"),
            "rendered text must contain 'world'; got: {text:?}"
        );
    }

    // =========================================================================
    // §9.5 Depth-increment mutant-kill tests (SEC-001)
    //
    // Each test calls a normalizer directly with a fixture constructed so the
    // FIRST recursive call lands at exactly depth == MAX_ADF_DEPTH (256).
    // Under correct code (`depth + 1`) the guard fires → Err.
    // Under the surviving mutant (`depth * 1`) the depth never increments,
    // the guard never fires, and the function returns Ok — so the test would
    // FAIL.  Asserting Err here kills the mutant.
    // =========================================================================

    /// Kills the `depth + 1` → `depth * 1` mutant at the
    /// `normalize_list_item_content(inner, depth + 1)?` call (blockquote-in-listItem
    /// arm, ~line 1680).
    ///
    /// A `blockquote` node inside the children list triggers that arm; calling with
    /// `depth = MAX_ADF_DEPTH - 1` means the recursive call gets depth 256 → Err.
    #[test]
    fn test_normalize_list_item_content_depth_increment_kills_mutant() {
        // A blockquote node with one paragraph inside.  The listItem normalizer
        // unwraps the blockquote and calls itself recursively on its children.
        let blockquote_node = json!({
            "type": "blockquote",
            "content": [
                {"type": "paragraph", "content": [{"type": "text", "text": "x"}]}
            ]
        });
        let result = normalize_list_item_content(vec![blockquote_node], MAX_ADF_DEPTH - 1);
        assert!(
            result.is_err(),
            "normalize_list_item_content must Err when recursive call hits depth 256; got Ok"
        );
        assert_eq!(
            result.unwrap_err().exit_code(),
            64,
            "depth guard must produce exit_code 64"
        );
    }

    /// Kills the `depth + 1` → `depth * 1` mutant at the
    /// `normalize_list_item_content(vec![ti], depth + 1)?` call (nested taskList
    /// inside listItem.content, ~line 1713).
    ///
    /// A `taskList` child whose own content contains another `taskList` node
    /// triggers the `ti["type"] == "taskList"` branch, which recurses.  With
    /// `depth = MAX_ADF_DEPTH - 1` the recursive call gets depth 256 → Err.
    #[test]
    fn test_normalize_list_item_content_tasklist_depth_increment_kills_mutant() {
        // A taskList node whose content contains another taskList (triggers the
        // `if ti["type"] == "taskList"` recursion branch inside the outer taskList arm).
        let nested_task_list = json!({
            "type": "taskList",
            "content": [
                {
                    "type": "taskList",
                    "content": [
                        {
                            "type": "taskItem",
                            "attrs": {"localId": "1", "state": "TODO"},
                            "content": [{"type": "text", "text": "inner"}]
                        }
                    ]
                }
            ]
        });
        let result = normalize_list_item_content(vec![nested_task_list], MAX_ADF_DEPTH - 1);
        assert!(
            result.is_err(),
            "normalize_list_item_content must Err when nested taskList recurse hits depth 256; got Ok"
        );
        assert_eq!(
            result.unwrap_err().exit_code(),
            64,
            "depth guard must produce exit_code 64"
        );
    }

    /// Kills the `depth + 1` → `depth * 1` mutant at the
    /// `normalize_blockquote_content(vec![ti], depth + 1)?` call (nested taskList
    /// inside a blockquote-level taskList, ~line 1822).
    ///
    /// A `taskList` node whose content contains another `taskList` triggers the
    /// `Some("taskList")` inner arm, which recurses.  With `depth = MAX_ADF_DEPTH - 1`
    /// the recursive call gets depth 256 → Err.
    #[test]
    fn test_normalize_blockquote_content_depth_increment_kills_mutant() {
        // A taskList node whose items include another taskList, triggering the
        // recursive `normalize_blockquote_content(vec![ti], depth + 1)?` call.
        let nested_task_list = json!({
            "type": "taskList",
            "content": [
                {
                    "type": "taskList",
                    "content": [
                        {
                            "type": "taskItem",
                            "attrs": {"localId": "1", "state": "TODO"},
                            "content": [{"type": "text", "text": "inner"}]
                        }
                    ]
                }
            ]
        });
        let result = normalize_blockquote_content(vec![nested_task_list], MAX_ADF_DEPTH - 1);
        assert!(
            result.is_err(),
            "normalize_blockquote_content must Err when nested taskList recurse hits depth 256; got Ok"
        );
        assert_eq!(
            result.unwrap_err().exit_code(),
            64,
            "depth guard must produce exit_code 64"
        );
    }

    /// Kills the `depth + 1` → `depth * 1` mutant at the
    /// `normalize_panel_content(inner, depth + 1)?` call (nested panel/blockquote
    /// inside panel content, ~line 1874).
    ///
    /// A `blockquote` node inside panel content triggers the
    /// `Some("panel") | Some("blockquote")` arm, which recurses.  With
    /// `depth = MAX_ADF_DEPTH - 1` the recursive call gets depth 256 → Err.
    #[test]
    fn test_normalize_panel_content_depth_increment_kills_mutant() {
        // A blockquote node inside panel content triggers the recursive arm.
        let blockquote_node = json!({
            "type": "blockquote",
            "content": [
                {"type": "paragraph", "content": [{"type": "text", "text": "x"}]}
            ]
        });
        let result = normalize_panel_content(vec![blockquote_node], MAX_ADF_DEPTH - 1);
        assert!(
            result.is_err(),
            "normalize_panel_content must Err when blockquote-in-panel recurse hits depth 256; got Ok"
        );
        assert_eq!(
            result.unwrap_err().exit_code(),
            64,
            "depth guard must produce exit_code 64"
        );
    }

    /// Kills the `depth + 1` → `depth * 1` mutant at the
    /// `autolink_bare_urls(content, depth + 1)?` call (container node `_` arm,
    /// ~line 239).
    ///
    /// A container node that is neither `"text"` nor `"codeBlock"` and carries a
    /// `"content"` array triggers the recursive call.  A `paragraph` node with a
    /// text child satisfies this.  Calling with `depth = MAX_ADF_DEPTH - 1` means
    /// the recursive call gets depth 256 → Err.
    #[test]
    fn test_autolink_bare_urls_depth_increment_kills_mutant() {
        // A paragraph node with content triggers the `_ => { recurse }` arm.
        let mut nodes = vec![json!({
            "type": "paragraph",
            "content": [{"type": "text", "text": "hello"}]
        })];
        let result = autolink_bare_urls(&mut nodes, MAX_ADF_DEPTH - 1);
        assert!(
            result.is_err(),
            "autolink_bare_urls must Err when container-node recurse hits depth 256; got Ok"
        );
        assert_eq!(
            result.unwrap_err().exit_code(),
            64,
            "depth guard must produce exit_code 64"
        );
    }

    /// O-1: Pin that plain-text interior lines inside a `<div>` HTML block are
    /// preserved as part of the same HtmlBlock (one paragraph) with alternating
    /// text/hardBreak nodes per Algorithm B (BC-7.2.011, issues #489/#492).
    ///
    /// Non-tautology: this test would fail if pulldown-cmark stopped treating
    /// plain-text continuation lines as part of the type-6 HTML block and instead
    /// split them into a separate paragraph node (giving a different top-level node
    /// count), OR if Algorithm B merged adjacent plain-text segments (yielding a
    /// single large text node instead of discrete per-line text nodes separated by
    /// hardBreaks). The existing `test_convert_multiline_block_html_preserves_interior_newlines`
    /// uses HTML-tag-bearing interior lines (`  <span>x</span>`); this test pins
    /// the same guarantee for PLAIN-TEXT interior lines ("line one", "line two"),
    /// closing the coverage gap that the D4 holdout scenario O-1 relies on.
    #[test]
    fn test_block_html_plain_text_interior_lines_preserved_in_one_paragraph() {
        // BC-7.2.011 Algorithm B (O-1 regression pin).
        // Input: a type-6 HTML block whose interior lines are plain prose, not HTML.
        // CommonMark §4.6: the block continues until a blank line, so all four lines
        // belong to a single HtmlBlock event.  Algorithm B then segments them.
        let adf = markdown_to_adf("<div>\nline one\nline two\n</div>").unwrap();

        // Exactly one top-level node: a paragraph (not split into separate blocks).
        let content = adf["content"].as_array().unwrap();
        assert_eq!(
            content.len(),
            1,
            "all four lines must be one paragraph, not separate blocks: {adf}"
        );
        assert_eq!(
            content[0]["type"], "paragraph",
            "the single block must be a paragraph: {adf}"
        );

        // Algorithm B: 4 segments → 3 boundaries → 7 nodes
        // [text("<div>"), hardBreak, text("line one"), hardBreak,
        //  text("line two"), hardBreak, text("</div>")]
        let para_content = content[0]["content"].as_array().unwrap();
        assert_eq!(
            para_content.len(),
            7,
            "4 plain-text segments must produce 7 content nodes \
             (text, hb, text, hb, text, hb, text): {para_content:?}"
        );
        // Node sequence assertions (exact, positional).
        assert_eq!(
            para_content[0]["type"], "text",
            "node[0] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[0]["text"], "<div>",
            "node[0] text must be '<div>': {para_content:?}"
        );
        assert_eq!(
            para_content[1]["type"], "hardBreak",
            "node[1] must be hardBreak: {para_content:?}"
        );
        assert_eq!(
            para_content[2]["type"], "text",
            "node[2] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[2]["text"], "line one",
            "node[2] text must be 'line one': {para_content:?}"
        );
        assert_eq!(
            para_content[3]["type"], "hardBreak",
            "node[3] must be hardBreak: {para_content:?}"
        );
        assert_eq!(
            para_content[4]["type"], "text",
            "node[4] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[4]["text"], "line two",
            "node[4] text must be 'line two': {para_content:?}"
        );
        assert_eq!(
            para_content[5]["type"], "hardBreak",
            "node[5] must be hardBreak: {para_content:?}"
        );
        assert_eq!(
            para_content[6]["type"], "text",
            "node[6] must be text: {para_content:?}"
        );
        assert_eq!(
            para_content[6]["text"], "</div>",
            "node[6] text must be '</div>': {para_content:?}"
        );

        // BC-7.2.011 INV-1: no text node may contain a raw \n character.
        for node in para_content {
            if node["type"] == "text" {
                let text_val = node["text"].as_str().unwrap_or("");
                assert!(
                    !text_val.contains('\n'),
                    "text node must not contain raw \\n (BC-7.2.011 INV-1): {node:?}"
                );
                assert!(
                    !text_val.contains('\r'),
                    "text node must not contain raw \\r (BC-7.2.011 INV-1): {node:?}"
                );
            }
        }
    }

    /// O-3: Pin that a footnote reference and its definition produce DISCRETE,
    /// UNMARKED text nodes — not merged text and not mark-inheriting nodes.
    ///
    /// Non-tautology: this test would fail if a future refactor merged adjacent
    /// same-paragraph text nodes (a valid ADF-equivalence optimization, e.g. to
    /// reduce node count), because `ref_nodes.len() == 2` and the exact-equality
    /// assertion `ref_nodes[1]["text"] == "[1]"` require `"[1]"` to be its own
    /// separate node with no surrounding text merged in.  Mark-inheritance in a
    /// bold context is pinned separately by
    /// `test_markdown_footnote_reference_marker_does_not_inherit_marks`; the
    /// fixture here (`"See note.[^1]"`) has no surrounding marks, so that
    /// regression is not detectable here.
    /// The existing concatenated-text assertions in `test_markdown_footnote_reference_renders_marker_not_literal_caret`
    /// and `test_markdown_footnote_definition_appended_after_rule_with_label` verify
    /// that the strings appear somewhere, but do NOT verify node-level discreteness or
    /// mark absence at the node level; this test closes that gap.
    #[test]
    fn test_footnote_reference_and_definition_are_discrete_unmarked_text_nodes() {
        // BC-7.2.011 / issue #472 (O-3 regression pin).
        let adf = markdown_to_adf("See note.[^1]\n\n[^1]: The note body.").unwrap();
        let content = adf["content"].as_array().unwrap();

        // --- Reference paragraph (content[0]) ---
        let ref_para = &content[0];
        assert_eq!(
            ref_para["type"], "paragraph",
            "content[0] must be a paragraph: {adf}"
        );
        let ref_nodes = ref_para["content"].as_array().unwrap();

        // Exactly two text nodes in the reference paragraph: "See note." and "[1]".
        assert_eq!(
            ref_nodes.len(),
            2,
            "reference paragraph must contain exactly 2 text nodes: {ref_nodes:?}"
        );

        // First node: the prose text — no marks.
        assert_eq!(
            ref_nodes[0]["type"], "text",
            "ref_para node[0] must be text: {ref_nodes:?}"
        );
        assert_eq!(
            ref_nodes[0]["text"], "See note.",
            "ref_para node[0] text: {ref_nodes:?}"
        );
        assert!(
            ref_nodes[0].get("marks").is_none(),
            "ref_para node[0] must carry no marks: {ref_nodes:?}"
        );

        // Second node: the reference marker — a SEPARATE text node carrying NO marks.
        assert_eq!(
            ref_nodes[1]["type"], "text",
            "ref_para node[1] must be text: {ref_nodes:?}"
        );
        assert_eq!(
            ref_nodes[1]["text"], "[1]",
            "reference marker must be exactly '[1]': {ref_nodes:?}"
        );
        assert!(
            ref_nodes[1].get("marks").is_none(),
            "footnote reference marker must carry NO marks (must not inherit active marks): {ref_nodes:?}"
        );

        // --- Rule divider (content[1]) ---
        assert_eq!(
            content[1]["type"], "rule",
            "content[1] must be the footnotes-section rule divider: {adf}"
        );

        // --- Definition paragraph (content[2]) ---
        let def_para = &content[2];
        assert_eq!(
            def_para["type"], "paragraph",
            "content[2] must be the definition paragraph: {adf}"
        );
        let def_nodes = def_para["content"].as_array().unwrap();

        // Exactly two text nodes: "[1] " prefix and "The note body.".
        assert_eq!(
            def_nodes.len(),
            2,
            "definition paragraph must contain exactly 2 text nodes: {def_nodes:?}"
        );

        // First node: the label prefix — a DISCRETE "[1] " node, NO marks.
        assert_eq!(
            def_nodes[0]["type"], "text",
            "def_para node[0] must be text: {def_nodes:?}"
        );
        assert_eq!(
            def_nodes[0]["text"], "[1] ",
            "definition label prefix must be exactly '[1] ' (with trailing space): {def_nodes:?}"
        );
        assert!(
            def_nodes[0].get("marks").is_none(),
            "definition label prefix must carry no marks: {def_nodes:?}"
        );

        // Second node: the definition body — separate node, NO marks.
        assert_eq!(
            def_nodes[1]["type"], "text",
            "def_para node[1] must be text: {def_nodes:?}"
        );
        assert_eq!(
            def_nodes[1]["text"], "The note body.",
            "definition body text: {def_nodes:?}"
        );
        assert!(
            def_nodes[1].get("marks").is_none(),
            "definition body must carry no marks: {def_nodes:?}"
        );
    }
}

# adf_to_text Rich Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the free `render_node`/`render_children` pair in `src/adf.rs` with a stateful `AdfRenderer` struct supporting ordered-list numbering (respecting `attrs.order`), inline marks (`strong`/`em`/`strike`/`code`/`link`), blockquote line prefixing (including nested `> > `), `rule` as `---`, `hardBreak` as `\n`, `codeBlock` with language fence, pipe-style tables with header separator, and graceful fallback for unknown nodes.

**Architecture:** Stateful visitor struct `AdfRenderer { output, list_stack }` mirroring the existing write-path `AdfBuilder` pattern. Blockquote prefixing uses a split-and-prefix pass after rendering children into `output`, so all descendant content gets prefixed uniformly. Marks apply inside-out by iterating `node.marks[]` in array order (last mark outermost). Tables emit pipe-row markdown with a `| --- | --- |` separator line after any row containing a `tableHeader` cell.

**Tech Stack:** Rust 2024 (edition), MSRV 1.85, `serde_json::Value` for ADF traversal, `insta` for snapshots. No new crate dependencies.

---

### Task 1: Scaffold `AdfRenderer` preserving current behavior

**Files:**
- Modify: `src/adf.rs` (replace `adf_to_text`, `render_node`, `render_children` at lines 345–410 with struct + impl; keep public signature `pub fn adf_to_text(&Value) -> String`)

Pure refactor. No new behavior. All existing tests must still pass after this task.

- [ ] **Step 1: Replace the free-function read path with a struct-based one**

In `src/adf.rs`, replace the range from line 345 (start of `pub fn adf_to_text`) through line 410 (end of `fn render_children`) with:

```rust
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
    // `Ordered { next_index: u64 }` variant added in Task 2 alongside its first use.
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
                if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
                    self.output.push_str(text);
                }
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
            "bulletList" | "orderedList" => {
                self.list_stack.push(ListFrame::Bullet);
                self.render_children(node);
                self.list_stack.pop();
            }
            "listItem" => {
                let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));
                self.output.push_str(&indent);
                self.output.push_str("- ");
                self.render_children(node);
            }
            "codeBlock" => {
                self.output.push_str("```\n");
                self.render_children(node);
                self.output.push_str("\n```\n");
            }
            _ => {
                if node.get("content").is_some() {
                    self.render_children(node);
                } else {
                    self.output
                        .push_str(&format!("[unsupported: {node_type}]"));
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

    fn finish(self) -> String {
        self.output.trim_end().to_string()
    }
}
```

Note: `ListFrame` has a single `Bullet` variant in Task 1; the `Ordered` variant gets added in Task 2 alongside its first use. This keeps Task 1 a minimal behavior-preserving refactor.

- [ ] **Step 2: Run existing tests — all must pass**

Run: `cargo test --lib adf::` from the repo root.

Expected: all `adf::tests::*` tests pass, including `test_adf_to_text_paragraph`, `test_adf_roundtrip_heading`, `test_adf_to_text_unsupported`, `test_adf_to_text_snapshot`.

- [ ] **Step 3: Run clippy to catch any warnings introduced**

Run: `cargo clippy --all-targets -- -D warnings`

Expected: no warnings. If the unused `Ordered` variant warns, that's fine — Task 2 uses it. If a different warning appears, fix before committing.

- [ ] **Step 4: Commit**

```bash
git add src/adf.rs
git commit -m "refactor: port adf_to_text to AdfRenderer struct (#202)

Pure refactor preserving existing behavior. Introduces the state
carrier needed for ordered-list numbering and blockquote nesting
in subsequent tasks. Mirrors the write-path AdfBuilder pattern."
```

---

### Task 2: Ordered-list numeric prefixes with `attrs.order`

**Files:**
- Modify: `src/adf.rs` — update the `bulletList | orderedList` and `listItem` match arms; add tests

- [ ] **Step 1: Write failing tests**

Append to the `#[cfg(test)] mod tests` block in `src/adf.rs`:

```rust
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
        // Jira treats order < 1 as start-at-1 (matches HTML <ol start> behavior).
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib adf::tests::test_render_ordered -- --exact` and the mixed-list test individually.

Expected: 4 failures — listItem uses `- ` for everything, ignoring the parent frame.

- [ ] **Step 3: Update `bulletList | orderedList` and `listItem` arms**

Replace the `"bulletList" | "orderedList"` arm with two separate arms:

```rust
            "bulletList" => {
                self.list_stack.push(ListFrame::Bullet);
                self.render_children(node);
                self.list_stack.pop();
            }
            "orderedList" => {
                let start = node
                    .get("attrs")
                    .and_then(|a| a.get("order"))
                    .and_then(|o| o.as_u64())
                    .filter(|&n| n >= 1)
                    .unwrap_or(1);
                self.list_stack.push(ListFrame::Ordered { next_index: start });
                self.render_children(node);
                self.list_stack.pop();
            }
            // NOTE: Task 2 also adds the `Ordered` variant to the `ListFrame` enum:
            //     enum ListFrame {
            //         Bullet,
            //         Ordered { next_index: u64 },
            //     }
```

Replace the `"listItem"` arm with:

```rust
            "listItem" => {
                let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));
                self.output.push_str(&indent);
                // Determine the prefix from the enclosing list frame, then
                // increment the counter so the NEXT sibling item gets the next number.
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib adf::` — all old + 4 new tests pass.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings` — no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/adf.rs
git commit -m "feat: render orderedList with numeric prefixes (#202)

Respects attrs.order; treats order<1 as start-at-1 matching Jira's
renderer. Nested bulletList inside orderedList keeps its indent."
```

---

### Task 3: Inline marks (`strong`/`em`/`strike`/`code`/`link`)

**Files:**
- Modify: `src/adf.rs` — extend `"text"` arm to apply marks; add tests

- [ ] **Step 1: Write failing tests**

Append to the test module:

```rust
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
        // marks = [strong, em] applied in order: strong wraps first (inner), em wraps second (outer).
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "foo", "marks": [{"type": "strong"}, {"type": "em"}]}
            ]}]
        });
        // strong-then-em → "*" + "**foo**" + "*" = "***foo***"
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib adf::tests::test_render_strong_mark adf::tests::test_render_em_mark adf::tests::test_render_strike_mark adf::tests::test_render_code_mark adf::tests::test_render_link`

Expected: all fail — marks are currently ignored.

- [ ] **Step 3: Add a mark-application helper and update the `"text"` arm**

Add a free helper near the bottom of the non-test code in `src/adf.rs` (just above `#[cfg(test)]`):

```rust
/// Wrap `text` with markdown-style syntax for each mark, innermost-first.
/// Unknown mark types pass through without added syntax.
fn apply_marks(text: &str, marks: Option<&Vec<Value>>) -> String {
    let mut result = text.to_string();
    let Some(marks) = marks else { return result };
    for mark in marks {
        let mark_type = mark.get("type").and_then(|t| t.as_str()).unwrap_or("");
        result = match mark_type {
            "code" => format!("`{result}`"),
            "em" => format!("*{result}*"),
            "strong" => format!("**{result}**"),
            "strike" => format!("~~{result}~~"),
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
```

Replace the `"text"` arm in `render_node` with:

```rust
            "text" => {
                let text = node.get("text").and_then(|t| t.as_str()).unwrap_or("");
                let marks = node.get("marks").and_then(|m| m.as_array());
                self.output.push_str(&apply_marks(text, marks));
            }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib adf::` — all old + 8 new mark tests pass.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings` — no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/adf.rs
git commit -m "feat: render inline marks on text nodes (#202)

Adds strong/em/strike/code/link. Marks iterate in marks[] array
order, wrapping inner-to-outer. Unknown mark types pass through
as bare text."
```

---

### Task 4: Blockquote line prefixing (including nested `> > `)

**Files:**
- Modify: `src/adf.rs` — add a `"blockquote"` arm with a split-and-prefix pass; add tests

- [ ] **Step 1: Write failing tests**

Append to the test module:

```rust
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
        // Each rendered line inside the blockquote starts with "> ".
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib adf::tests::test_render_blockquote adf::tests::test_render_nested_blockquote`

Expected: both fail — no blockquote prefix is currently emitted (the existing code's `_` arm just recurses into the content).

- [ ] **Step 3: Add the `"blockquote"` arm**

Insert a new arm in `render_node` above the fallback `_`:

```rust
            "blockquote" => {
                let start = self.output.len();
                self.render_children(node);

                // Prefix every line in the just-rendered segment with "> ".
                // Internal blank lines get bare ">" (no trailing space),
                // trailing empties are trimmed. Nested blockquotes accumulate
                // to "> > " automatically because each outer level re-prefixes
                // its inner level's already-prefixed output on unwind — no
                // depth counter is needed.
                let rendered = self.output.split_off(start);
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
```

Note on the mechanics: each `blockquote` arm pops its rendered segment, splits on `\n`, prefixes each non-empty line with `"> "`, and writes it back. For `blockquote > blockquote > paragraph`, the inner blockquote runs first on its unwind → produces `"> inner\n"`. The outer then processes that text and prefixes each line again → produces `"> > inner\n"`. Exactly the CommonMark-compliant `> > ` nested form.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib adf::` — all tests pass including the two new ones.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings` — no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/adf.rs
git commit -m "feat: render blockquote with > line prefixes (#202)

Every line inside a blockquote gets a '> ' prefix. Nested
blockquotes accumulate to '> > ' per CommonMark convention."
```

---

### Task 5: `rule`, `hardBreak`, codeBlock with language

**Files:**
- Modify: `src/adf.rs` — add `"rule"` and `"hardBreak"` arms, extend `"codeBlock"` for `attrs.language`; add tests

- [ ] **Step 1: Write failing tests**

Append to the test module:

```rust
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
        assert!(text.contains("```rust"), "expected rust fence, got: {text:?}");
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
        assert!(text.contains("```\nplain"), "expected empty fence, got: {text:?}");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib adf::tests::test_render_rule adf::tests::test_render_hard_break adf::tests::test_render_code_block`

Expected: `test_render_rule`, `test_render_hard_break_inserts_newline`, and `test_render_code_block_with_language` fail. `test_render_code_block_without_language` may pass already.

- [ ] **Step 3: Add arms and extend codeBlock**

Add two new arms above the fallback `_`:

```rust
            "rule" => {
                self.output.push_str("---\n");
            }
            "hardBreak" => {
                self.output.push('\n');
            }
```

Replace the existing `"codeBlock"` arm with:

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib adf::`

All tests pass, including 4 new ones.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings` — no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/adf.rs
git commit -m "feat: render rule, hardBreak, and codeBlock language (#202)

rule → '---' on its own line.
hardBreak → plain '\n' (the trailing-two-spaces form is markdown
source syntax, not rendered-text convention).
codeBlock → fence opens with the attrs.language when present."
```

---

### Task 6: Tables (pipe rows with header separator)

**Files:**
- Modify: `src/adf.rs` — add `"table"`, `"tableRow"`, `"tableCell"`, `"tableHeader"` arms; add tests

- [ ] **Step 1: Write failing tests**

Append to the test module:

```rust
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
        assert!(text.contains("| --- | --- |"), "separator missing: {text:?}");
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
        assert!(text.contains("| --- | --- |"), "separator missing: {text:?}");
    }

    #[test]
    fn test_render_table_cell_flattens_paragraph() {
        // Paragraph inside cell should not emit its own trailing newline
        // (would break the | cell | cell | row structure).
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib adf::tests::test_render_table`

Expected: 3 failures — tables currently fall through to the `_` recurse-into-content arm, which emits raw text without pipe structure.

- [ ] **Step 3: Add table-family arms**

Add these four arms above the fallback `_`:

```rust
            "table" => {
                self.render_children(node);
                self.output.push('\n');
            }
            "tableRow" => {
                let cells = node
                    .get("content")
                    .and_then(|c| c.as_array())
                    .cloned()
                    .unwrap_or_default();
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
```

Add this new helper method on `impl AdfRenderer`:

```rust
    /// Render a tableCell/tableHeader's children in "flat" mode: a paragraph's
    /// inline content is emitted without its trailing newline (which would
    /// break the "| cell | cell |" row structure). Other block types inside
    /// a cell (rare but legal per the schema) fall back to normal rendering
    /// with best-effort whitespace collapsing to keep the row on one line.
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
                    // Inline children only — no trailing newline.
                    if let Some(cc) = child.get("content").and_then(|c| c.as_array()) {
                        for inline in cc {
                            self.render_node(inline);
                        }
                    }
                }
                _ => self.render_node(child),
            }
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib adf::`

All tests pass, including 3 new table tests.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings` — no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/adf.rs
git commit -m "feat: render ADF tables as pipe rows with header separator (#202)

Separator emitted after any row that contains a tableHeader cell
(matches ADF schema where header-ness is per-cell, not per-row).
Paragraph inside a cell is flattened to keep the row on one line."
```

---

### Task 7: Graceful fallback — drop unknown leaves silently

**Files:**
- Modify: `src/adf.rs` — change the fallback arm; rewrite the existing `test_adf_to_text_unsupported` test

- [ ] **Step 1: Update the existing test and add a new companion test**

Locate `test_adf_to_text_unsupported` (currently at `src/adf.rs:460`). Replace it with these two:

```rust
    #[test]
    fn test_render_unknown_leaf_drops_silently() {
        // Leaf (no content array) of an unknown type should produce empty text,
        // not debug junk like "[unsupported: mediaGroup]".
        let adf = json!({
            "type": "doc",
            "content": [{ "type": "mediaGroup" }]
        });
        assert_eq!(adf_to_text(&adf), "");
    }

    #[test]
    fn test_render_unknown_container_recurses() {
        // Container (has content array) of an unknown type should render its
        // children — salvages panel, nestedExpand, etc.
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
```

- [ ] **Step 2: Run tests to verify the new leaf-drop test fails (container test already passes)**

Run: `cargo test --lib adf::tests::test_render_unknown`

Expected: `test_render_unknown_leaf_drops_silently` fails — current code emits `[unsupported: mediaGroup]`. `test_render_unknown_container_recurses` passes already because the current fallback already recurses into `content`.

- [ ] **Step 3: Change the fallback arm**

In `render_node`, replace the `_` arm's body:

```rust
            _ => {
                // Unknown node: recurse into content if present, otherwise
                // drop silently. Per the #202 spec, this avoids debug strings
                // like "[unsupported: type]" reaching user output while still
                // salvaging the text content of container nodes like panel or
                // nestedExpand.
                if node.get("content").is_some() {
                    self.render_children(node);
                }
            }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib adf::`

All tests pass, including both fallback tests.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings` — no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/adf.rs
git commit -m "fix: drop unknown ADF leaf nodes silently (#202)

Replaces the '[unsupported: <type>]' debug string with empty
output. Unknown containers still recurse into content. Matches
the flexydox/adf-to-md precedent and avoids leaking internal
debug syntax into 'jr issue view' output."
```

---

### Task 8: Snapshot regeneration + roundtrip test

**Files:**
- Modify: `src/adf.rs` — rewrite `test_adf_to_text_snapshot` with a rich input, add `test_markdown_to_adf_to_text_roundtrip`
- Modify: `src/snapshots/jr__adf__tests__adf_to_text_complex.snap` (auto-regenerated by `cargo insta review`)

- [ ] **Step 1: Replace the existing snapshot test**

Locate `test_adf_to_text_snapshot` (currently around line 766). Replace its body with a rich input that exercises the new rendering paths:

```rust
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
```

- [ ] **Step 2: Add the roundtrip test**

Append to the test module:

```rust
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

        // Compare node-type sequences at each depth for structural equivalence.
        // Marks are compared as sets per text node (CommonMark renders
        // '{strong, em}' and '{em, strong}' identically as '***text***').
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
```

- [ ] **Step 3: Run tests; review and accept the new snapshot**

Run: `cargo test --lib adf::`

The first run will produce a snapshot mismatch (the existing `.snap` file is for the old, simpler input). Review and accept:

```bash
cargo insta review
# In the review UI: press `a` to accept the new snapshot, `q` to quit.
# Or non-interactively:
cargo insta accept --unreferenced=delete
```

Then re-run: `cargo test --lib adf::` — all tests pass.

- [ ] **Step 4: Run the full CI-equivalent check set**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All three pass.

- [ ] **Step 5: Commit**

```bash
git add src/adf.rs src/snapshots/jr__adf__tests__adf_to_text_complex.snap
git commit -m "test: snapshot + roundtrip coverage for rich adf_to_text (#202)

Regenerates the adf_to_text_complex snapshot with a rich input
exercising headings, mixed inline marks, nested mixed-list,
blockquote, rule, codeBlock-with-language, and a 2-row table.
Adds a markdown→ADF→text→ADF roundtrip test comparing node-type
sequences for structural equivalence."
```

---

## Self-Review

**Spec coverage:**
- Ordered lists with `attrs.order` → Task 2 ✅
- Nested list indentation → Tasks 2 (mixed-list test), 8 (snapshot) ✅
- Table pipe format → Task 6 ✅
- Link href preservation → Task 3 ✅
- Inline marks (strong/em/strike/code) → Task 3 ✅
- Blockquote `> ` prefix → Task 4 ✅
- Rule → Task 5 ✅
- CodeBlock language → Task 5 ✅
- HardBreak → Task 5 ✅
- Graceful fallback (recurse/drop) → Task 7 ✅
- Snapshot update → Task 8 ✅
- Roundtrip test → Task 8 ✅
- `listItem` no trailing `\n` → encoded in Task 1's scaffold (matches existing behavior) ✅
- Unknown marks pass bare → Task 3 (`test_render_unknown_mark_drops_syntax`) ✅

**No placeholders:** searched for TBD/TODO/"fill in"/"add appropriate"/"handle edge cases" — none present.

**Type consistency:**
- `AdfRenderer` fields: `output: String`, `list_stack: Vec<ListFrame>` — consistent across tasks. `blockquote_depth` was scoped out during implementation (stack-unwind re-prefix needs no depth counter).
- `ListFrame` variants: `Bullet`, `Ordered { next_index: u64 }` — consistent.
- Method names: `new`, `render_doc`, `render_node`, `render_children`, `render_cell_inline`, `finish` — consistent.
- `apply_marks(&str, Option<&Vec<Value>>)` helper signature used only in Task 3's `"text"` arm.

No gaps found.

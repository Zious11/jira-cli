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
    let mut content: Vec<Value> = Vec::new();
    let mut in_code_block = false;
    let mut code_lines: Vec<String> = Vec::new();
    let mut list_items: Vec<Value> = Vec::new();

    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            if in_code_block {
                content.push(json!({
                    "type": "codeBlock",
                    "content": [{ "type": "text", "text": code_lines.join("\n") }]
                }));
                code_lines.clear();
                in_code_block = false;
            } else {
                flush_list(&mut list_items, &mut content);
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_lines.push(line.to_string());
            continue;
        }

        if !line.trim_start().starts_with("- ") && !list_items.is_empty() {
            flush_list(&mut list_items, &mut content);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(heading) = parse_heading(trimmed) {
            content.push(heading);
        } else if let Some(stripped) = trimmed.strip_prefix("- ") {
            list_items.push(json!({
                "type": "listItem",
                "content": [{
                    "type": "paragraph",
                    "content": parse_inline(stripped)
                }]
            }));
        } else {
            content.push(json!({
                "type": "paragraph",
                "content": parse_inline(trimmed)
            }));
        }
    }

    flush_list(&mut list_items, &mut content);

    json!({ "version": 1, "type": "doc", "content": content })
}

fn flush_list(items: &mut Vec<Value>, content: &mut Vec<Value>) {
    if !items.is_empty() {
        content.push(json!({
            "type": "bulletList",
            "content": std::mem::take(items)
        }));
    }
}

fn parse_heading(line: &str) -> Option<Value> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&level) && line.len() > level && line.as_bytes()[level] == b' ' {
        let text = &line[level + 1..];
        Some(json!({
            "type": "heading",
            "attrs": { "level": level },
            "content": [{ "type": "text", "text": text }]
        }))
    } else {
        None
    }
}

fn parse_inline(text: &str) -> Vec<Value> {
    let mut result = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if let Some(pos) = remaining.find("**") {
            if pos > 0 {
                result.push(json!({"type": "text", "text": &remaining[..pos]}));
            }
            let after = &remaining[pos + 2..];
            if let Some(end) = after.find("**") {
                result.push(json!({
                    "type": "text", "text": &after[..end],
                    "marks": [{"type": "strong"}]
                }));
                remaining = &after[end + 2..];
                continue;
            }
        }

        if let Some(pos) = remaining.find('`') {
            if pos > 0 {
                result.push(json!({"type": "text", "text": &remaining[..pos]}));
            }
            let after = &remaining[pos + 1..];
            if let Some(end) = after.find('`') {
                result.push(json!({
                    "type": "text", "text": &after[..end],
                    "marks": [{"type": "code"}]
                }));
                remaining = &after[end + 1..];
                continue;
            }
        }

        result.push(json!({"type": "text", "text": remaining}));
        break;
    }

    if result.is_empty() {
        result.push(json!({"type": "text", "text": text}));
    }

    result
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
        let input = "## Root cause\n\nThe auth module had a **critical bug** in `validate_token`.\n\n- Missing null check\n- Wrong error type\n\n```rust\nfn validate() -> bool {\n    true\n}\n```";
        let adf = markdown_to_adf(input);
        insta::assert_json_snapshot!("markdown_complex_to_adf", adf);
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
}
